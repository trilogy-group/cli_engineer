use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use log::{debug, error};

use crate::llm_manager::LLMProvider;
use crate::event_bus::{Event, EventBus};

/// xAI API provider implementation
pub struct XAIProvider {
    api_key: String,
    model: String,
    base_url: String,
    temperature: f32,
    event_bus: Option<Arc<EventBus>>,
    cost_per_1m_input_tokens: f32,
    cost_per_1m_output_tokens: f32,
}

#[derive(Debug, Serialize)]
struct XAIRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct XAIResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    created: u64,
    #[allow(dead_code)]
    model: String,
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    #[allow(dead_code)]
    index: u32,
    message: ResponseMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl XAIProvider {
    /// Create a new xAI provider with default settings
    pub fn new(model: Option<String>, temperature: Option<f32>) -> Result<Self> {
        let api_key = env::var("XAI_API_KEY").context("XAI_API_KEY environment variable not set")?;
        
        Ok(Self::with_config(
            api_key,
            model.unwrap_or_else(|| "grok-beta".to_string()),
        )
        .with_temperature(temperature.unwrap_or(0.7)))
    }

    /// Create a new xAI provider with custom configuration
    pub fn with_config(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.x.ai/v1".to_string(),
            temperature: 0.7,
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
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set event bus for event handling
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Set cost per 1 million input tokens
    pub fn with_cost_per_1m_input_tokens(mut self, cost: f32) -> Self {
        self.cost_per_1m_input_tokens = cost;
        self
    }

    /// Set cost per 1 million output tokens
    pub fn with_cost_per_1m_output_tokens(mut self, cost: f32) -> Self {
        self.cost_per_1m_output_tokens = cost;
        self
    }
}

#[async_trait]
impl LLMProvider for XAIProvider {
    fn name(&self) -> &str {
        "xAI"
    }

    fn context_size(&self) -> usize {
        match self.model.as_str() {
            "grok-beta" => 131_072,
            "grok-2-1212" => 131_072,
            "grok-2-vision-1212" => 131_072,
            _ => 32_768, // Default conservative limit
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
        
        let request = XAIRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a helpful AI assistant.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: self.temperature,
            max_tokens: None,
        };

        debug!("Sending request to xAI API with model: {}", self.model);
        
        let response = client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to xAI API")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            error!("xAI API error (status {}): {}", status, response_text);
            return Err(anyhow!(
                "xAI API error (status {}): {}", 
                status, 
                response_text
            ));
        }

        let response: XAIResponse = serde_json::from_str(&response_text)
            .with_context(|| format!("Failed to parse xAI response: {}", response_text))?;

        if let Some(choice) = response.choices.first() {
            // Emit usage metrics if available
            if let Some(usage) = &response.usage {
                if let Some(event_bus) = &self.event_bus {
                    let total_cost = (usage.prompt_tokens as f32 * self.cost_per_1m_input_tokens / 1_000_000.0)
                        + (usage.completion_tokens as f32 * self.cost_per_1m_output_tokens / 1_000_000.0);
                    
                    let _ = event_bus.emit(Event::APICallCompleted {
                        provider: "xAI".to_string(),
                        tokens: usage.total_tokens as usize,
                        cost: total_cost,
                    }).await;
                }
            }

            Ok(choice.message.content.clone())
        } else {
            Err(anyhow!("No response from xAI API"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_sizes() {
        let provider = XAIProvider::with_config("test".to_string(), "grok-beta".to_string());
        assert_eq!(provider.context_size(), 131_072);
        
        let provider = XAIProvider::with_config("test".to_string(), "grok-2-1212".to_string());
        assert_eq!(provider.context_size(), 131_072);
        
        let provider = XAIProvider::with_config("test".to_string(), "unknown".to_string());
        assert_eq!(provider.context_size(), 32_768);
    }
}
