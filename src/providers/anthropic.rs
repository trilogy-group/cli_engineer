use async_trait::async_trait;
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use log::{debug, error};
use futures::stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader;

use crate::llm_manager::LLMProvider;
use crate::event_bus::{Event, EventBus};

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: usize,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinking>,
}

#[derive(Debug, Serialize)]
struct AnthropicThinking {
    #[serde(rename = "type")]
    thinking_type: String,
    budget_tokens: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicContent {
    text: String,
    #[serde(rename = "type")]
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: usize,
    output_tokens: usize,
}

// Streaming event structures
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { 
        #[allow(dead_code)]
        message: StreamMessage 
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { 
        content_block: ContentBlock 
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { 
        delta: ContentDelta 
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop,
    #[serde(rename = "message_delta")]
    MessageDelta { 
        delta: MessageDelta 
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamMessage {
    id: String,
    #[serde(rename = "type")]
    message_type: String,
    role: String,
    content: Vec<serde_json::Value>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentDelta {
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { 
        #[allow(dead_code)]
        signature: String 
    },
}

#[derive(Debug, Deserialize)]
struct MessageDelta {
    #[allow(dead_code)]
    stop_reason: Option<String>,
    #[allow(dead_code)]
    stop_sequence: Option<String>,
    usage: Option<AnthropicUsage>,
}

/// Anthropic Claude API provider implementation
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: Client,
    temperature: f32,
    event_bus: Option<Arc<EventBus>>,
    cost_per_1m_input_tokens: f32,
    cost_per_1m_output_tokens: f32,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider instance
    pub fn new(
        api_key: String,
        model: String,
        temperature: f32,
        cost_per_1m_input_tokens: f32,
        cost_per_1m_output_tokens: f32,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            model,
            temperature,
            cost_per_1m_input_tokens,
            cost_per_1m_output_tokens,
            event_bus,
        }
    }

    /// Check if the current model supports extended thinking
    fn supports_extended_thinking(&self) -> bool {
        self.model.starts_with("claude-sonnet-4") ||
        self.model.starts_with("claude-opus-4") ||
        self.model.starts_with("claude-haiku-4")
    }

    /// Calculate the cost for API usage
    fn calculate_cost(&self, input_tokens: usize, output_tokens: usize) -> f32 {
        (input_tokens as f32 * self.cost_per_1m_input_tokens / 1_000_000.0) + 
        (output_tokens as f32 * self.cost_per_1m_output_tokens / 1_000_000.0)
    }

    /// Log token usage with detailed breakdown
    fn log_usage(&self, input_tokens: usize, output_tokens: usize, cost: f32) {
        debug!(
            "Anthropic API usage - Input tokens: {}, Output tokens: {}, Total tokens: {}, Cost: ${:.4}",
            input_tokens, output_tokens, input_tokens + output_tokens, cost
        );
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

    fn handles_own_metrics(&self) -> bool {
        true
    }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let supports_thinking = self.supports_extended_thinking();
        
        let request = AnthropicRequest {
            model: self.model.clone(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 64000, // 64k output tokens per response
            temperature: if supports_thinking { 1.0 } else { self.temperature },
            stream: Some(true),
            thinking: if supports_thinking {
                Some(AnthropicThinking {
                    thinking_type: "enabled".to_string(),
                    budget_tokens: 10000, // Allow up to 10k tokens for thinking
                })
            } else {
                None
            },
        };

        debug!("Sending Anthropic request with streaming and thinking: {}", supports_thinking);

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

        // Process the streaming response
        let stream = response.bytes_stream();
        let stream_reader = StreamReader::new(stream.map(|result| {
            result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        }));
        let mut lines = FramedRead::new(stream_reader, LinesCodec::new());

        let mut final_text = String::new();
        let mut total_input_tokens = 0;
        let mut total_output_tokens = 0;
        
        // Thinking buffer state
        let mut thinking_buffer = String::new();
        let mut sent_thinking_length = 0;

        while let Some(line) = lines.next().await {
            let line = line.context("Failed to read line from stream")?;
            
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse server-sent event format
            if let Some(data_part) = line.strip_prefix("data: ") {
                if data_part == "[DONE]" {
                    debug!("Stream completed with [DONE] marker");
                    break;
                }
                
                debug!("Raw streaming event data: {}", data_part);

                match serde_json::from_str::<StreamEvent>(data_part) {
                    Ok(event) => {
                        match event {
                            StreamEvent::MessageStart { message } => {
                                debug!("Stream started");
                                debug!("MessageStart usage data: {:?}", message.usage);
                                if let Some(usage) = message.usage {
                                    debug!("Adding tokens from MessageStart - Input: {}, Output: {}", usage.input_tokens, usage.output_tokens);
                                    total_input_tokens += usage.input_tokens;
                                    total_output_tokens += usage.output_tokens;
                                } else {
                                    debug!("No usage data in MessageStart event");
                                }
                            }
                            StreamEvent::ContentBlockStart { content_block, .. } => {
                                match content_block {
                                    ContentBlock::Thinking { thinking } => {
                                        debug!("Thinking block started: {}", thinking);
                                        thinking_buffer.push_str(&thinking);
                                        
                                        // Handle initial thinking content with buffering
                                        if let Some(bus) = &self.event_bus {
                                            if thinking_buffer.len() > 200 {
                                                let trace_to_send = format!("🤔 {}", thinking_buffer.trim_end());
                                                sent_thinking_length = thinking_buffer.len();
                                                
                                                let bus_clone = bus.clone();
                                                tokio::spawn(async move {
                                                    let _ = bus_clone.emit(Event::ReasoningTrace { 
                                                        message: trace_to_send 
                                                    }).await;
                                                });
                                            }
                                        }
                                    }
                                    ContentBlock::Text { text } => {
                                        debug!("Text block started: {}", text);
                                        final_text.push_str(&text);
                                    }
                                }
                            }
                            StreamEvent::ContentBlockDelta { delta, .. } => {
                                match delta {
                                    ContentDelta::ThinkingDelta { thinking } => {
                                        debug!("Thinking delta: {}", thinking);
                                        thinking_buffer.push_str(&thinking);
                                        
                                        // Send chunks when buffer grows significantly OR at sentence boundaries
                                        if let Some(bus) = &self.event_bus {
                                            if thinking_buffer.len() > sent_thinking_length + 400 || 
                                               (thinking.contains(". ") || thinking.contains("! ") || thinking.contains("? ")) && 
                                               thinking_buffer.len() > sent_thinking_length + 50 {
                                                let new_content = &thinking_buffer[sent_thinking_length..];
                                                let cleaned_new = new_content.trim_end().to_string();
                                                if !cleaned_new.is_empty() {
                                                    let trace_to_send = if sent_thinking_length == 0 {
                                                        format!("🤔 {}", cleaned_new)
                                                    } else {
                                                        cleaned_new
                                                    };
                                                    sent_thinking_length = thinking_buffer.len();
                                                    
                                                    let bus_clone = bus.clone();
                                                    tokio::spawn(async move {
                                                        let _ = bus_clone.emit(Event::ReasoningTrace { 
                                                            message: trace_to_send 
                                                        }).await;
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    ContentDelta::TextDelta { text } => {
                                        debug!("Text delta: {}", text);
                                        final_text.push_str(&text);
                                    }
                                    ContentDelta::SignatureDelta { signature: _ } => {
                                        // Signature deltas are for cryptographic verification, we don't need to display them
                                        debug!("Received signature delta");
                                    }
                                }
                            }
                            StreamEvent::ContentBlockStop => {
                                debug!("Content block stopped");
                            }
                            StreamEvent::MessageDelta { delta } => {
                                debug!("Message delta received");
                                debug!("MessageDelta usage data: {:?}", delta.usage);
                                if let Some(usage) = delta.usage {
                                    debug!("Adding tokens from MessageDelta - Input: {}, Output: {}", usage.input_tokens, usage.output_tokens);
                                    total_input_tokens += usage.input_tokens;
                                    total_output_tokens += usage.output_tokens;
                                } else {
                                    debug!("No usage data in MessageDelta event");
                                }
                            }
                            StreamEvent::MessageStop => {
                                debug!("Message stream stopped");
                                break;
                            }
                            StreamEvent::Ping => {
                                debug!("Received ping event");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse stream event: {} - Data: {}", e, data_part);
                        // Continue processing other events instead of failing
                    }
                }
            } else if line.starts_with("event: ") {
                // Event type line, we can log it for debugging
                debug!("Stream event type: {}", line);
            }
        }

        // Send any remaining thinking content
        if !thinking_buffer.is_empty() && sent_thinking_length < thinking_buffer.len() {
            if let Some(bus) = &self.event_bus {
                let remaining_content = &thinking_buffer[sent_thinking_length..];
                let cleaned_remaining = remaining_content.trim().to_string();
                if !cleaned_remaining.is_empty() {
                    let trace_to_send = if sent_thinking_length == 0 {
                        format!("🤔 {}", cleaned_remaining)
                    } else {
                        format!("{}\n✨", cleaned_remaining)
                    };
                    
                    let bus_clone = bus.clone();
                    tokio::spawn(async move {
                        let _ = bus_clone.emit(Event::ReasoningTrace { 
                            message: trace_to_send 
                        }).await;
                    });
                }
            }
        }

        debug!("Final token counts - Input: {}, Output: {}", total_input_tokens, total_output_tokens);

        let cost = self.calculate_cost(total_input_tokens, total_output_tokens);
        self.log_usage(total_input_tokens, total_output_tokens, cost);

        // Emit an event with usage data
        if let Some(event_bus) = &self.event_bus {
            let _ = event_bus.emit(Event::APICallCompleted {
                provider: "anthropic".to_string(),
                tokens: total_input_tokens + total_output_tokens,
                cost,
            }).await;
        }

        if final_text.is_empty() {
            return Err(anyhow!("No text content received from Anthropic streaming response"));
        }

        Ok(final_text)
    }
}
