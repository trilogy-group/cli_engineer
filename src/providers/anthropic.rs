use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use log::warn;

use crate::llm_manager::LLMProvider;

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: usize,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
    #[serde(rename = "type")]
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[allow(dead_code)]
    input_tokens: usize,
    #[allow(dead_code)]
    output_tokens: usize,
}

/// Anthropic Claude API provider implementation
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: Client,
    max_tokens: usize,
    temperature: f32,
}

impl AnthropicProvider {
    pub fn new(model: Option<String>, temperature: Option<f32>) -> Result<Self> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY environment variable not set")?;

        Ok(Self {
            api_key,
            model: model.unwrap_or_else(|| "claude-opus-4-0".to_string()),
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: Client::new(),
            max_tokens: 8192,
            temperature: temperature.unwrap_or(0.2),
        })
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    #[allow(dead_code)]
    pub fn with_config(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: Client::new(),
            max_tokens,
            temperature: 0.7,
        }
    }

    #[allow(dead_code)]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "Anthropic"
    }

    fn context_size(&self) -> usize {
        // Context sizes for different Claude models
        match self.model.as_str() {
            "claude-3-opus-20240229" => 200_000,
            "claude-3-sonnet-20240229" => 200_000,
            "claude-3-haiku-20240307" => 200_000,
            "claude-2.1" => 100_000,
            "claude-2.0" => 100_000,
            _ => 100_000, // Default fallback
        }
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Anthropic API error: {}", error_text));
        }

        let api_response: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic API response")?;

        // Check if response was truncated
        if let Some(stop_reason) = &api_response.stop_reason {
            match stop_reason.as_str() {
                "max_tokens" => {
                    warn!("Anthropic response was truncated due to max_tokens limit ({}). Response may be incomplete.", self.max_tokens);
                }
                "end_turn" => {
                    // Normal completion, no issues
                }
                other => {
                    warn!("Anthropic response stopped with reason: {}", other);
                }
            }
        }

        // Extract text from the first content block
        api_response
            .content
            .into_iter()
            .find(|c| c.content_type == "text")
            .map(|c| c.text)
            .ok_or_else(|| anyhow!("No text content in Anthropic response"))
    }
}
