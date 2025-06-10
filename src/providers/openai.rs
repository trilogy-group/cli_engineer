use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use log::{info, warn};

use crate::llm_manager::LLMProvider;
use crate::event_bus::{Event, EventBus};

/// OpenAI API provider implementation
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: usize,
    temperature: f32,
    event_bus: Option<Arc<EventBus>>,
    cost_per_1m_input_tokens: f32,
    cost_per_1m_output_tokens: f32,
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<usize>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
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
struct OpenAIError {
    error: OpenAIErrorDetails,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorDetails {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    code: Option<String>,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with default settings
    pub fn new(model: Option<String>, temperature: Option<f32>) -> Result<Self> {
        let api_key =
            env::var("OPENAI_API_KEY").context("OPENAI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.unwrap_or_else(|| "gpt-4.1".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            max_tokens: 8192,
            temperature: temperature.unwrap_or(0.2),
            event_bus: None,
            cost_per_1m_input_tokens: 0.0,
            cost_per_1m_output_tokens: 0.0,
        })
    }

    /// Create a new OpenAI provider with custom configuration
    #[allow(dead_code)]
    pub fn with_config(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.openai.com/v1".to_string(),
            max_tokens,
            temperature: 1.0, // Use default temperature of 1.0 for OpenAI models
            event_bus: None,
            cost_per_1m_input_tokens: 0.0,
            cost_per_1m_output_tokens: 0.0,
        }
    }

    /// Set custom base URL (for API-compatible services)
    #[allow(dead_code)]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Set temperature for response generation
    #[allow(dead_code)]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set event bus for event handling
    #[allow(dead_code)]
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Set cost per 1 million input tokens
    #[allow(dead_code)]
    pub fn with_cost_per_1m_input_tokens(mut self, cost: f32) -> Self {
        self.cost_per_1m_input_tokens = cost;
        self
    }

    /// Set cost per 1 million output tokens
    #[allow(dead_code)]
    pub fn with_cost_per_1m_output_tokens(mut self, cost: f32) -> Self {
        self.cost_per_1m_output_tokens = cost;
        self
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "OpenAI"
    }

    fn context_size(&self) -> usize {
        match self.model.as_str() {
            "gpt-4o" | "gpt-4o-mini" => 128_000,
            "gpt-4-turbo" => 128_000,
            "gpt-4" => 8_192,
            "gpt-3.5-turbo" => 16_385,
            _ => 4_096, // Conservative default
        }
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn handles_own_metrics(&self) -> bool {
        true
    }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();

        // Check if model uses new max_completion_tokens parameter
        let uses_new_param = self.model.starts_with("gpt-4-")
            || self.model.starts_with("gpt-4o")
            || self.model.starts_with("o1")
            || self.model == "o4-mini";

        let request = OpenAIRequest {
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
            max_tokens: if uses_new_param {
                None
            } else {
                Some(self.max_tokens)
            },
            max_completion_tokens: if uses_new_param {
                Some(self.max_tokens)
            } else {
                None
            },
            temperature: self.temperature,
        };

        let response = client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<OpenAIError>(&response_text) {
                return Err(anyhow!(
                    "OpenAI API error: {} (type: {}, code: {:?})",
                    error_response.error.message,
                    error_response.error.error_type,
                    error_response.error.code
                ));
            } else {
                return Err(anyhow!(
                    "OpenAI API error (status {}): {}",
                    status,
                    response_text
                ));
            }
        }

        let openai_response: OpenAIResponse =
            serde_json::from_str(&response_text).context("Failed to parse OpenAI response")?;

        let choice = openai_response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No response choices from OpenAI"))?;

        let content = choice.message.content.clone();

        // Check if response was truncated
        if let Some(finish_reason) = &choice.finish_reason {
            match finish_reason.as_str() {
                "length" => {
                    warn!("OpenAI response was truncated due to max_tokens limit ({}). Response may be incomplete.", self.max_tokens);
                }
                "stop" => {
                    // Normal completion, no issues
                }
                other => {
                    warn!("OpenAI response finished with reason: {}", other);
                }
            }
        }

        // Log token usage if available
        if let Some(usage) = openai_response.usage {
            info!(
                "OpenAI token usage - Prompt: {}, Completion: {}, Total: {}",
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens
            );

            // Calculate cost using configured pricing
            let input_cost = (usage.prompt_tokens as f32 * self.cost_per_1m_input_tokens) / 1_000_000.0;
            let output_cost = (usage.completion_tokens as f32 * self.cost_per_1m_output_tokens) / 1_000_000.0;
            let total_cost = input_cost + output_cost;

            // Emit APICallCompleted event with accurate token counts and cost
            if let Some(event_bus) = &self.event_bus {
                let _ = event_bus.emit(Event::APICallCompleted {
                    provider: "openai".to_string(),
                    tokens: usage.total_tokens,
                    cost: total_cost,
                }).await;
            }
        }

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_sizes() {
        let provider =
            OpenAIProvider::with_config("test_key".to_string(), "gpt-4o".to_string(), 1000);
        assert_eq!(provider.context_size(), 128_000);

        let provider =
            OpenAIProvider::with_config("test_key".to_string(), "gpt-3.5-turbo".to_string(), 1000);
        assert_eq!(provider.context_size(), 16_385);
    }
}
