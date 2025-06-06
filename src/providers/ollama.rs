use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use log::warn;

use crate::llm_manager::LLMProvider;

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    choices: Vec<OllamaChoice>,
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<OllamaUsage>,
}

#[derive(Debug, Deserialize)]
struct OllamaChoice {
    message: OllamaMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaUsage {
    #[allow(dead_code)]
    prompt_tokens: Option<usize>,
    #[allow(dead_code)]
    completion_tokens: Option<usize>,
    #[allow(dead_code)]
    total_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OllamaError {
    error: OllamaErrorDetails,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorDetails {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

/// Ollama local LLM provider implementation
pub struct OllamaProvider {
    model: String,
    base_url: String,
    client: Client,
    max_tokens: usize,
    temperature: f32,
}

impl OllamaProvider {
    /// Create a new Ollama provider with default settings
    pub fn new(
        model: Option<String>,
        temperature: Option<f32>,
        base_url: Option<String>,
        max_tokens: Option<usize>,
    ) -> Result<Self> {
        Ok(Self {
            model: model.unwrap_or_else(|| "qwen3:8b".to_string()),
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            client: Client::new(),
            max_tokens: max_tokens.unwrap_or(8192),
            temperature: temperature.unwrap_or(0.7),
        })
    }

    /// Create a new Ollama provider with custom configuration
    #[allow(dead_code)]
    pub fn with_config(model: String, base_url: String, max_tokens: usize) -> Self {
        Self {
            model,
            base_url,
            client: Client::new(),
            max_tokens,
            temperature: 0.7,
        }
    }

    /// Set temperature for response generation
    #[allow(dead_code)]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set model name
    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// Set base URL for Ollama server
    #[allow(dead_code)]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn name(&self) -> &str {
        "Ollama"
    }

    fn context_size(&self) -> usize {
        // Context sizes for Ollama models (2024-2025)
        // Handle both base model names and size variants (e.g., "qwen3:8b")
        let base_model = self.model.split(':').next().unwrap_or(&self.model);

        match base_model {
            // Latest DeepSeek models
            "deepseek-r1" => 131_072, // 128K context
            "deepseek-v3" => 64_000,  // 64K context
            "deepseek-v2.5" => 64_000,
            "deepseek-coder-v2" => 64_000,

            // Latest Qwen models
            "qwen3" => 128_000,       // 128K for 8B+, 32K for smaller
            "qwen2.5" => 32_768,      // 32K context
            "qwen2.5-coder" => 32_768,
            "qwq" => 32_768,          // Reasoning model

            // Latest Llama models
            "llama4" => 128_000,      // Latest multimodal
            "llama3.3" => 128_000,    // 128K context
            "llama3.2" => 128_000,    // 128K context
            "llama3.1" => 128_000,    // 128K context
            "llama3" => 8_192,        // Original 8K
            "llama2" => 4_096,        // 4K context

            // Latest Microsoft Phi models
            "phi4" => 16_384,         // 16K context
            "phi4-mini" => 128_000,   // 128K context
            "phi4-reasoning" => 16_384,
            "phi3.5" => 128_000,      // 128K context
            "phi3" => 128_000,        // 128K context

            // Latest Google Gemma models
            "gemma3" => 128_000,      // 128K for larger, 32K for 1B
            "gemma2" => 8_192,        // 8K context

            // Code-specialized models
            "codellama" => 16_384,    // 16K context
            "codegemma" => 8_192,     // 8K context
            "devstral" => 128_000,    // 128K context

            // Mistral models
            "mistral" => 32_768,      // 32K context
            "mistral-nemo" => 128_000, // 128K context
            "mistral-small" => 128_000,
            "mistral-large" => 128_000,

            // Other notable models
            "granite3.2" => 128_000,  // IBM Granite
            "granite3.1-dense" => 128_000,
            "smollm2" => 8_192,       // Small models
            "tinyllama" => 2_048,     // Very small

            _ => 32_768, // More generous default for newer models
        }
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let request = OllamaRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: "You are a helpful AI assistant for coding tasks.".to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            max_tokens: Some(self.max_tokens),
            temperature: self.temperature,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Ollama")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<OllamaError>(&response_text) {
                return Err(anyhow!(
                    "Ollama API error: {} (type: {:?}, code: {:?})",
                    error_response.error.message,
                    error_response.error.error_type,
                    error_response.error.code
                ));
            } else {
                return Err(anyhow!(
                    "Ollama API error (status {}): {}",
                    status,
                    response_text
                ));
            }
        }

        let api_response: OllamaResponse = serde_json::from_str(&response_text)
            .context("Failed to parse Ollama API response")?;

        // Check if response was truncated
        if let Some(choice) = api_response.choices.first() {
            if let Some(finish_reason) = &choice.finish_reason {
                match finish_reason.as_str() {
                    "length" => {
                        warn!("Ollama response was truncated due to max_tokens limit ({}). Response may be incomplete.", self.max_tokens);
                    }
                    "stop" => {
                        // Normal completion, no issues
                    }
                    other => {
                        warn!("Ollama response finished with reason: {}", other);
                    }
                }
            }

            Ok(choice.message.content.clone())
        } else {
            Err(anyhow!("No choices in Ollama response"))
        }
    }
}
