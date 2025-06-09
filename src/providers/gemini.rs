use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::env;

use crate::llm_manager::LLMProvider;

/// Gemini API provider implementation
pub struct GeminiProvider {
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: usize,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    temperature: f32,
    reasoning_effort: String,
    stream: bool,
    include_thoughts: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    error: GeminiErrorDetails,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorDetails {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    code: Option<String>,
}

impl GeminiProvider {
    /// Create a new Gemini provider with default settings
    pub fn new(model: Option<String>, temperature: Option<f32>) -> Result<Self> {
        let api_key =
            env::var("GEMINI_API_KEY").context("GEMINI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.unwrap_or_else(|| "gemini-1.5-flash-latest".to_string()),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            max_tokens: 8192,
            temperature: temperature.unwrap_or(0.2),
        })
    }
}

#[async_trait]
impl LLMProvider for GeminiProvider {
    fn name(&self) -> &str {
        "Gemini"
    }

    fn context_size(&self) -> usize {
        match self.model.as_str() {
            "gemini-1.5-pro-latest" | "models/gemini-1.5-pro-preview-0514" => 1_048_576,
            "gemini-1.5-flash-latest" | "models/gemini-1.5-flash-preview-0514" => 1_048_576,
            _ => 1_048_576, // Default to 1M tokens for Gemini models
        }
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();

        let request = GeminiRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are a helpful AI assistant for coding tasks.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            max_tokens: Some(self.max_tokens),
            temperature: self.temperature,
            reasoning_effort: "low".to_string(),
            stream: false,
            include_thoughts: false,
        };

        let response = client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Gemini")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            if let Ok(error_response) = serde_json::from_str::<GeminiError>(&response_text) {
                return Err(anyhow!(
                    "Gemini API error: {} (type: {}, code: {:?})",
                    error_response.error.message,
                    error_response.error.error_type,
                    error_response.error.code
                ));
            } else {
                return Err(anyhow!(
                    "Gemini API error (status {}): {}",
                    status,
                    response_text
                ));
            }
        }

        let gemini_response: GeminiResponse =
            serde_json::from_str(&response_text).context("Failed to parse Gemini response")?;

        let choice = gemini_response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No response choices from Gemini"))?;

        let content = choice.message.content.clone();

        if let Some(finish_reason) = &choice.finish_reason {
            if finish_reason == "max_tokens" {
                warn!("Gemini response was truncated due to max_tokens limit ({}). Response may be incomplete.", self.max_tokens);
            }
        }

        if let Some(usage) = gemini_response.usage {
            info!(
                "Gemini token usage - Prompt: {}, Completion: {}, Total: {}",
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens
            );
        }

        Ok(content)
    }
}
