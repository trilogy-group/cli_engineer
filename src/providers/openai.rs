use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use log::{debug, error};

use crate::llm_manager::LLMProvider;
use crate::event_bus::{Event, EventBus};

/// OpenAI API provider implementation
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
    temperature: f32,
    event_bus: Option<Arc<EventBus>>,
    cost_per_1m_input_tokens: f32,
    cost_per_1m_output_tokens: f32,
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<OpenAIReasoning>,
}

#[derive(Debug, Serialize)]
struct OpenAIReasoning {
    summary: String, // "auto" or "detailed"
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    created_at: u64,
    #[serde(default)]
    #[allow(dead_code)]
    status: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    error: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    incomplete_details: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    instructions: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    max_output_tokens: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    model: Option<String>,
    output: Vec<ResponseMessage>,
    #[serde(default)]
    #[allow(dead_code)]
    parallel_tool_calls: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    previous_response_id: Option<String>,
    #[serde(default)]
    reasoning: Option<ResponseReasoning>,
    #[serde(default)]
    #[allow(dead_code)]
    store: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    temperature: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    text: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    tool_choice: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    #[allow(dead_code)]
    top_p: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    truncation: Option<String>,
    #[serde(default)]
    usage: Option<Usage>,
    #[serde(default)]
    #[allow(dead_code)]
    user: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    message_type: String,
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    status: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Vec<ContentItem>>,
    #[serde(default)]
    #[allow(dead_code)]
    summary: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct ContentItem {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    annotations: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ResponseReasoning {
    #[allow(dead_code)]
    effort: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: usize,
    #[serde(default)]
    #[allow(dead_code)]
    input_tokens_details: Option<serde_json::Value>,
    output_tokens: usize,
    #[serde(default)]
    #[allow(dead_code)]
    output_tokens_details: Option<serde_json::Value>,
    total_tokens: usize,
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
            temperature: temperature.unwrap_or(0.2),
            event_bus: None,
            cost_per_1m_input_tokens: 0.0,
            cost_per_1m_output_tokens: 0.0,
        })
    }

    /// Create a new OpenAI provider with custom configuration
    #[allow(dead_code)]
    pub fn with_config(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            base_url: "https://api.openai.com/v1".to_string(),
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

    fn is_reasoning_model(model: &str) -> bool {
        model.starts_with("o1") || model.starts_with("o3") || model.starts_with("o4-mini")
    }

    /// Helper function to emit reasoning summary in chunks for better dashboard display
    async fn emit_reasoning_summary_chunks(&self, summary: &str) {
        if let Some(event_bus) = &self.event_bus {
            // Split by sentences first, then by chunks if sentences are too long
            let sentences: Vec<&str> = summary.split(". ").collect();
            let mut current_chunk = String::new();
            const MAX_CHUNK_SIZE: usize = 200; // Similar to Ollama's approach

            for (i, sentence) in sentences.iter().enumerate() {
                let sentence_with_period = if i < sentences.len() - 1 && !sentence.ends_with('.') {
                    format!("{}. ", sentence)
                } else {
                    sentence.to_string()
                };

                // If adding this sentence would exceed chunk size, emit current chunk
                if !current_chunk.is_empty() && current_chunk.len() + sentence_with_period.len() > MAX_CHUNK_SIZE {
                    let _ = event_bus
                        .emit(Event::ReasoningTrace {
                            message: current_chunk.trim().to_string(),
                        })
                        .await;
                    current_chunk.clear();
                }

                current_chunk.push_str(&sentence_with_period);
            }

            // Emit any remaining content
            if !current_chunk.trim().is_empty() {
                let _ = event_bus
                    .emit(Event::ReasoningTrace {
                        message: current_chunk.trim().to_string(),
                    })
                    .await;
            }
        }
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

        // Check if this is a reasoning model that supports reasoning summaries
        let is_reasoning_model = Self::is_reasoning_model(&self.model);

        let request = OpenAIRequest {
            model: self.model.clone(),
            input: prompt.to_string(),
            reasoning: if is_reasoning_model {
                Some(OpenAIReasoning {
                    summary: "detailed".to_string(),
                })
            } else {
                None
            },
        };

        let response = client
            .post(format!("{}/responses", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI API")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("OpenAI API error: {}", error_text));
        }

        let response_text = response.text().await?;
        debug!("Raw OpenAI response: {}", response_text);
        
        // Try to parse as pretty JSON first for better debugging
        if let Ok(pretty_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            debug!("Raw response as JSON: {}", serde_json::to_string_pretty(&pretty_json).unwrap_or_default());
        }

        let openai_response: OpenAIResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                error!("Failed to parse OpenAI response. Error: {}", e);
                error!("Raw response was: {}", response_text);
                anyhow::anyhow!("Failed to parse OpenAI response: {}", e)
            })?;

        debug!("Parsed OpenAI response: {:?}", openai_response);

        let content = openai_response.output.iter().find_map(|item| {
            if item.message_type == "message" {
                item.content.as_ref().and_then(|content| {
                    content.iter().find_map(|content_item| {
                        if content_item.content_type == "text" || content_item.content_type == "output_text" {
                            Some(content_item.text.clone())
                        } else {
                            None
                        }
                    })
                })
            } else {
                None
            }
        }).unwrap_or_default();

        // Handle reasoning summary for reasoning models
        if let Some(reasoning) = &openai_response.reasoning {
            if let Some(summary) = &reasoning.summary {
                self.emit_reasoning_summary_chunks(summary).await;
            }
        }

        // Also check for reasoning summary in output items (for reasoning models)
        for item in &openai_response.output {
            if item.message_type == "reasoning" {
                if let Some(summary_items) = &item.summary {
                    let summary_text: Vec<String> = summary_items
                        .iter()
                        .filter_map(|item| {
                            item.get("text").and_then(|v| v.as_str()).map(|s| s.to_string())
                        })
                        .collect();
                    
                    if !summary_text.is_empty() {
                        let combined_summary = summary_text.join("\n\n");
                        self.emit_reasoning_summary_chunks(&combined_summary).await;
                    }
                }
            }
        }

        // Log token usage if available
        if let Some(usage) = openai_response.usage {
            // Calculate cost using configured pricing
            let input_cost = (usage.input_tokens as f32 * self.cost_per_1m_input_tokens) / 1_000_000.0;
            let output_cost = (usage.output_tokens as f32 * self.cost_per_1m_output_tokens) / 1_000_000.0;
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
            OpenAIProvider::with_config("test_key".to_string(), "gpt-4o".to_string());
        assert_eq!(provider.context_size(), 128_000);

        let provider =
            OpenAIProvider::with_config("test_key".to_string(), "gpt-3.5-turbo".to_string());
        assert_eq!(provider.context_size(), 16_385);
    }
}
