//! ToolManager – central registry for all tools available to the agent.
//!
//! Phase-2 introduces Model-Context-Protocol (MCP) tools.  ToolManager is
//! responsible for:
//!   • Reading `Config.mcp.servers`.
//!   • Spawning local stdio MCP servers (if `command` provided).
//!   • Building an [`MCPClient`] for each server.
//!   • Discovering tools via `/v1/tools` and exposing them through a unified
//!     API for the rest of the application.
//!
//! NOTE: for the first cut we only support JSON request / JSON response
//! (non-streaming).  A streaming wrapper can be added once needed.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use log::warn;

use crate::config::Config;
use crate::mcp::{UnifiedMCPClient, MCPClient, StdioMCPClient, ToolDescriptor};

/// Handle that binds a tool descriptor to the client that serves it
#[derive(Clone)]
pub struct ToolHandle {
    /// Tool descriptor from MCP server
    pub descriptor: ToolDescriptor,
    /// Arc-wrapped client for shared access
    client: Arc<UnifiedMCPClient>,
}

/// Central registry & router for tool invocations
pub struct ToolManager {
    tools: HashMap<String, ToolHandle>,
}

impl ToolManager {
    /// Build a ToolManager from configuration – blocking until discovery
    /// completes.  Spawns local servers as necessary.
    pub async fn from_config(cfg: &Config) -> Result<Self> {
        let mut registry = HashMap::new();

        if let Some(mcp_cfg) = &cfg.mcp {
            for server in &mcp_cfg.servers {
                if !server.enabled {
                    continue;
                }

                // Check if this is a stdio MCP server (has command but no base_url)
                let is_stdio_server = server.base_url.is_none() && server.command.is_some();
                
                // (1) Build unified client (either stdio or HTTP)
                let client = if is_stdio_server {
                    // Create stdio MCP client
                    let command = server.command.as_ref().unwrap();
                    match StdioMCPClient::new(command, &server.args, &server.env).await {
                        Ok(stdio_client) => Arc::new(UnifiedMCPClient::Stdio(stdio_client)),
                        Err(e) => {
                            warn!("Failed to create stdio MCP client for '{}': {}",
                                server.name.as_deref().unwrap_or("unknown"), e);
                            continue;
                        }
                    }
                } else {
                    // Create HTTP MCP client
                    let base_url = match &server.base_url {
                        Some(url) => url.clone(),
                        None => {
                            warn!("MCP server '{}' requires either `base_url` or `command`",
                                server.name.as_deref().unwrap_or("unknown"));
                            continue;
                        }
                    };
                    
                    match MCPClient::new(base_url, server.api_key.clone()) {
                        Ok(http_client) => Arc::new(UnifiedMCPClient::Http(http_client)),
                        Err(e) => {
                            warn!("Failed to create HTTP MCP client for '{}': {}",
                                server.name.as_deref().unwrap_or("unknown"), e);
                            continue;
                        }
                    }
                };

                // (3) Discover tools
                let tools = match client.list_tools().await {
                    Ok(tools) => tools,
                    Err(e) => {
                        warn!("Failed to list tools from MCP server '{}': {}",
                            server.name.as_deref().unwrap_or("unknown"), e);
                        continue;
                    }
                };

                for desc in tools {
                    let name = desc.name.clone();
                    registry.entry(name.clone()).or_insert_with(|| ToolHandle {
                        descriptor: desc,
                        client: client.clone(),
                    });
                }
            }
        }

        Ok(Self {
            tools: registry,
        })
    }

    /// Return names of all tools
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
    
    /// List all available tools with their descriptors
    pub fn list_tools(&self) -> Vec<ToolDescriptor> {
        self.tools.values().map(|handle| handle.descriptor.clone()).collect()
    }

    /// Invoke a tool by name with JSON args
    pub async fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let handle = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow!("Tool `{}` not found", name))?;

        handle.client.call_tool(name, args).await
    }
    
    /// Invoke a tool by name with JSON string args (wrapper for executor)
    pub async fn invoke_tool(
        &self,
        name: &str,
        args_str: &str,
    ) -> Result<String> {
        // Parse the JSON string to Value
        let args: serde_json::Value = serde_json::from_str(args_str)
            .with_context(|| format!("Failed to parse tool arguments for {}: {}", name, args_str))?;
        
        // Call the tool
        let result = self.call_tool(name, &args).await?;
        
        // Convert result back to string
        Ok(serde_json::to_string_pretty(&result)?)
    }
}


