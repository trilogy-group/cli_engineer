use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use futures_util::Stream;
use tokio_util::bytes::Bytes;
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{debug, info};
use tokio::time::{sleep, timeout, Duration};

/// Descriptor returned by an MCP server for each available tool
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    /// JSON Schema describing expected arguments
    pub parameters: Option<Value>,
}

/// JSON-RPC request message
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC response message
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error object
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[allow(dead_code)]
    data: Option<Value>,
}

/// Stdio MCP client for servers that communicate via JSON-RPC over stdin/stdout
#[derive(Debug)]
pub struct StdioMCPClient {
    child: Arc<Mutex<Child>>,
    request_id: Arc<Mutex<u64>>,
}

impl StdioMCPClient {
    /// Create a new stdio MCP client by spawning the given command
    pub async fn new(command: &str, args: &[String], env: &std::collections::HashMap<String, String>) -> Result<Self> {
        info!("Starting stdio MCP server: {} {:?}", command, args);
        
        let mut cmd = Command::new(command);
        cmd.args(args);
        
        // Set environment variables
        for (key, val) in env {
            let resolved = if val.starts_with("${") && val.ends_with('}') {
                std::env::var(&val[2..val.len() - 1]).unwrap_or_default()
            } else {
                val.clone()
            };
            cmd.env(key, resolved);
        }
        
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
            
        let child = cmd.spawn().context("failed to start stdio MCP server")?;
        
        let client = Self {
            child: Arc::new(Mutex::new(child)),
            request_id: Arc::new(Mutex::new(1)),
        };
        
        // Give the MCP server a moment to initialize
        info!("Waiting for stdio MCP server to initialize...");
        sleep(Duration::from_millis(500)).await;
        
        // Verify the server is responsive by sending an initialize request
        match timeout(Duration::from_secs(5), client.initialize()).await {
            Ok(Ok(_)) => {
                info!("Stdio MCP server initialized successfully");
                Ok(client)
            }
            Ok(Err(e)) => {
                info!("Failed to initialize stdio MCP server: {}", e);
                // Kill the child process if initialization fails
                let mut child = client.child.lock().await;
                let _ = child.kill().await;
                Err(e)
            }
            Err(_) => {
                info!("Timeout initializing stdio MCP server");
                // Kill the child process on timeout
                let mut child = client.child.lock().await;
                let _ = child.kill().await;
                Err(anyhow::anyhow!("Timeout waiting for MCP server to initialize"))
            }
        }
    }
    
    /// Initialize the MCP connection with the server
    async fn initialize(&self) -> Result<()> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": {
                    "listChanged": false
                }
            },
            "clientInfo": {
                "name": "cli_engineer",
                "version": "1.3.0"
            }
        });
        
        let _ = self.send_request("initialize", Some(params)).await?;
        Ok(())
    }
    
    /// Send a JSON-RPC request and receive the response
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = {
            let mut request_id = self.request_id.lock().await;
            let id = *request_id;
            *request_id += 1;
            id
        };
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        
        let mut child = self.child.lock().await;
        
        // Send request
        let request_json = serde_json::to_string(&request)?;
        debug!("Sending MCP request: {}", request_json);
        
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(request_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        } else {
            return Err(anyhow::anyhow!("child stdin unavailable"));
        }
        
        // Read response
        let mut response_line = String::new();
        if let Some(stdout) = child.stdout.as_mut() {
            let mut reader = BufReader::new(stdout);
            reader.read_line(&mut response_line).await?;
        } else {
            return Err(anyhow::anyhow!("child stdout unavailable"));
        }
        
        debug!("Received MCP response: {}", response_line.trim());
        
        // Log raw response for debugging
        if response_line.trim().is_empty() {
            return Err(anyhow::anyhow!("MCP server returned empty response"));
        }
        
        let response: JsonRpcResponse = serde_json::from_str(&response_line)
            .context("failed to parse JSON-RPC response")?;
            
        if let Some(error) = response.error {
            let error_details = if let Some(data) = &error.data {
                format!(" ({})", serde_json::to_string(data).unwrap_or_default())
            } else {
                String::new()
            };
            return Err(anyhow::anyhow!("MCP server error {}: {}{}", error.code, error.message, error_details));
        }
        
        response.result.ok_or_else(|| anyhow::anyhow!("MCP response missing result"))
    }
    
    /// List available tools from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<ToolDescriptor>> {
        let result = self.send_request("tools/list", Some(serde_json::json!({}))).await?;
        
        // The response should have a "tools" field containing the array
        if let Some(tools_value) = result.get("tools") {
            let tools: Vec<ToolDescriptor> = serde_json::from_value(tools_value.clone())
                .context("failed to parse tools list from MCP response")?;
            Ok(tools)
        } else {
            // Fallback: try to parse the result directly as an array
            let tools: Vec<ToolDescriptor> = serde_json::from_value(result)
                .context("failed to parse tools list from MCP response")?;
            Ok(tools)
        }
    }
    
    /// Invoke a tool on the MCP server
    pub async fn call_tool(&self, name: &str, args: &Value) -> Result<Value> {
        let params = serde_json::json!({
            "name": name,
            "arguments": args
        });
        
        self.send_request("tools/call", Some(params)).await
    }
}

/// Unified MCP client that can handle both HTTP and stdio servers
#[derive(Debug)]
pub enum UnifiedMCPClient {
    Http(MCPClient),
    Stdio(StdioMCPClient),
}

impl UnifiedMCPClient {
    /// List available tools from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<ToolDescriptor>> {
        match self {
            UnifiedMCPClient::Http(client) => client.list_tools().await,
            UnifiedMCPClient::Stdio(client) => client.list_tools().await,
        }
    }
    
    /// Invoke a tool on the MCP server
    pub async fn call_tool(&self, name: &str, args: &Value) -> Result<Value> {
        match self {
            UnifiedMCPClient::Http(client) => client.call_tool(name, args).await,
            UnifiedMCPClient::Stdio(client) => client.call_tool(name, args).await,
        }
    }
}

/// Lightweight async client for the Model Context Protocol (MCP)
/// This client is intentionally minimal – it implements only the subset of
/// the MCP spec required by cli_engineer in Phase-2.
///
/// Supported endpoints (relative to `base_url`):
///   GET  /v1/tools            -> list available tools
///   POST /v1/tools/{name}     -> invoke a tool (JSON body = args)
/// The exact paths may evolve; they map to the reference implementation used
/// by the OpenHands project.
#[derive(Debug, Clone)]
pub struct MCPClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl MCPClient {
    /// Create a new MCPClient with an optional bearer `api_key`.
    pub fn new<S: Into<String>>(base_url: S, api_key: Option<String>) -> Result<Self> {
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            http: reqwest::Client::builder()
                .user_agent("cli_engineer/1.0 (+https://cli.engineer)")
                .build()
                .context("failed to build reqwest client")?,
        })
    }

    fn auth_header(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &self.api_key {
            req.bearer_auth(key)
        } else {
            req
        }
    }

    /// Fetch the list of available tools from the remote server
    pub async fn list_tools(&self) -> Result<Vec<ToolDescriptor>> {
        let url = format!("{}/v1/tools", self.base_url);
        debug!("Fetching tools from HTTP MCP: {}", url);
        
        let resp = self
            .auth_header(self.http.get(&url))
            .send()
            .await
            .context("failed to query MCP /tools endpoint")?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("MCP server returned HTTP {}", status);
        }

        // Get the raw text first to debug what we're receiving
        let text = resp.text().await
            .context("failed to read response body")?;
        debug!("HTTP MCP response: {}", text);
        
        // Try to parse as JSON
        let tools: Vec<ToolDescriptor> = match serde_json::from_str(&text) {
            Ok(tools) => tools,
            Err(e) => {
                // Include first 200 chars of response in error for debugging
                let preview = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text.clone()
                };
                return Err(anyhow::anyhow!(
                    "invalid JSON from MCP /tools endpoint. Parse error: {}. Response preview: {}", 
                    e, preview
                ));
            }
        };
        Ok(tools)
    }

    /// Invoke a remote tool by name with the provided JSON args.
    pub async fn call_tool(&self, name: &str, args: &Value) -> Result<Value> {
        let url = format!("{}/v1/tools/{}", self.base_url, name);
        let resp = self
            .auth_header(self.http.post(url).json(args))
            .send()
            .await
            .with_context(|| format!("failed to invoke MCP tool {}", name))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("MCP tool {} failed with HTTP {} – {}", name, status, text);
        }
        
        // Parse the response as JSON
        resp.json().await
            .with_context(|| format!("failed to parse MCP tool {} response as JSON", name))
    }

    /// Streaming variant – returns a byte stream (SSE/Chunked).
    #[allow(dead_code)]
    pub async fn call_tool_stream(
        &self,
        name: &str,
        args: &Value,
    ) -> Result<impl Stream<Item = Result<Bytes, reqwest::Error>>> {
        let url = format!("{}/v1/tools/{}/stream", self.base_url, name);
        let resp = self
            .auth_header(self.http.post(url).json(args))
            .send()
            .await
            .with_context(|| format!("failed to invoke MCP stream tool {}", name))?;
        Ok(resp.bytes_stream())
    }
}
