use serde::{Deserialize, Serialize};
use std::env;
use std::str;
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use async_trait::async_trait;
use std::sync::Arc;

use crate::llm_manager::LLMProvider;
use crate::event_bus::{Event, EventBus};

/// Gemini API provider implementation
pub struct GeminiProvider {
    api_key: String,
    model: String,
    max_tokens: usize,
    temperature: f32,
    base_url: String,
    event_bus: Option<Arc<EventBus>>,
    cost_per_1m_input_tokens: f32,
    cost_per_1m_output_tokens: f32,
}

// Native Gemini API request format
#[derive(Serialize, Debug)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
    #[serde(rename = "systemInstruction")]
    system_instruction: Option<Content>,
}

#[derive(Serialize, Debug)]
struct Content {
    parts: Vec<Part>,
    role: Option<String>,
}

#[derive(Serialize, Debug)]
struct Part {
    text: String,
}

#[derive(Serialize, Debug)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: usize,
    #[serde(rename = "thinkingConfig")]
    thinking_config: Option<ThinkingConfig>,
}

#[derive(Serialize, Debug)]
struct ThinkingConfig {
    #[serde(rename = "includeThoughts")]
    include_thoughts: bool,
}

// Native Gemini API response format
#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize, Debug)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<usize>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<usize>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<ResponseContent>,
}

#[derive(Deserialize, Debug)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ResponsePart {
    text: Option<String>,
    #[serde(default)]
    thought: bool,
}

impl GeminiProvider {
    /// Create a new Gemini provider with default settings
    pub fn new(model: Option<String>, temperature: Option<f32>, cost_per_1m_input_tokens: Option<f32>, cost_per_1m_output_tokens: Option<f32>, event_bus: Option<Arc<EventBus>>) -> Result<Self> {
        let api_key =
            env::var("GEMINI_API_KEY").context("GEMINI_API_KEY environment variable not set")?;
        Ok(Self {
            api_key,
            model: model.unwrap_or_else(|| "gemini-1.5-flash-latest".to_string()),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            max_tokens: 8192,
            temperature: temperature.unwrap_or(0.2),
            event_bus,
            cost_per_1m_input_tokens: cost_per_1m_input_tokens.unwrap_or(0.0),
            cost_per_1m_output_tokens: cost_per_1m_output_tokens.unwrap_or(0.0),
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
    
    fn handles_own_metrics(&self) -> bool {
        true // Gemini provider uses direct API token counts and handles its own cost calculation
    }
    
    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();

        let request = GeminiRequest {
            contents: vec![
                Content {
                    parts: vec![Part {
                        text: prompt.to_string(),
                    }],
                    role: Some("user".to_string()),
                },
            ],
            generation_config: GenerationConfig {
                temperature: self.temperature,
                max_output_tokens: self.max_tokens,
                thinking_config: Some(ThinkingConfig {
                    include_thoughts: true,
                }),
            },
            system_instruction: Some(Content {
                parts: vec![Part {
                    text: "You are a helpful AI assistant for coding tasks.".to_string(),
                }],
                role: None,
            }),
        };

        // Use streaming endpoint for thinking support
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, self.model, self.api_key
        );

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Gemini")?;

        let status = response.status();
        
        if !status.is_success() {
            let response_text = response.text().await?;
            return Err(anyhow!(
                "Gemini API error (status {}): {}",
                status,
                response_text
            ));
        }

        // Handle streaming response
        let mut stream = response.bytes_stream();
        let mut full_content = String::new();
        let mut thinking_buffer = String::new();
        let mut total_prompt_tokens = 0;
        let mut total_candidates_tokens = 0;
        let mut total_tokens = 0;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Failed to read response chunk")?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            
            // Parse Server-Sent Events format
            for line in chunk_str.lines() {
                if line.starts_with("data: ") {
                    let json_data = &line[6..]; // Remove "data: " prefix
                    if json_data.trim() == "[DONE]" {
                        break;
                    }
                    
                    if let Ok(response_chunk) = serde_json::from_str::<GeminiResponse>(json_data) {
                        
                        // Accumulate token usage from each chunk
                        if let Some(usage) = &response_chunk.usage_metadata {
                            if let Some(prompt_tokens) = usage.prompt_token_count {
                                total_prompt_tokens = prompt_tokens; // This should be consistent across chunks
                            }
                            if let Some(candidates_tokens) = usage.candidates_token_count {
                                total_candidates_tokens = candidates_tokens; // This accumulates
                            }
                            if let Some(total) = usage.total_token_count {
                                total_tokens = total; // This accumulates
                            }
                        }
                        
                        if let Some(candidates) = &response_chunk.candidates {
                            for candidate in candidates {
                                if let Some(content) = &candidate.content {
                                    for part in &content.parts {
                                        if let Some(text) = &part.text {
                                            if part.thought {
                                                // This is thinking content - buffer it and emit reasoning traces
                                                thinking_buffer.push_str(text);
                                                
                                                // Split buffer into lines and emit them as reasoning traces
                                                for line in thinking_buffer.lines() {
                                                    if !line.trim().is_empty() {
                                                        if let Some(bus) = &self.event_bus {
                                                            let _ = bus.emit(Event::ReasoningTrace {
                                                                message: line.to_string(),
                                                            }).await;
                                                        }
                                                    }
                                                }
                                                thinking_buffer.clear(); // Clear buffer after processing lines
                                            } else {
                                                // This is regular response content
                                                full_content.push_str(text);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                    }
                }
            }
        }
        
        if full_content.is_empty() {
            return Err(anyhow!("Empty response from Gemini"));
        }

        // Emit APICallCompleted event with accurate token counts - let LLMManager handle cost calculation using config pricing
        if let Some(event_bus) = &self.event_bus {
            
            // Fix for Gemini 2.5 Pro Preview: candidates_token_count is often None, but total_token_count is available
            if total_candidates_tokens == 0 && total_tokens > 0 && total_prompt_tokens > 0 {
                // Calculate candidates tokens from total - prompt tokens
                total_candidates_tokens = total_tokens.saturating_sub(total_prompt_tokens);
            }
            
            // If we still don't have token counts, fall back to estimation
            if total_tokens == 0 {
                // Improved estimation: More accurate for thinking models
                // Research shows: ~3.5-4 characters per token for English text, ~3 for code/structured text
                total_prompt_tokens = ((prompt.len() as f32) / 3.5).ceil() as usize;
                
                // Include both regular content and ALL accumulated thinking content for output tokens
                let total_output_chars = full_content.len() + thinking_buffer.len();
                total_candidates_tokens = ((total_output_chars as f32) / 3.5).ceil() as usize;
                total_tokens = total_prompt_tokens + total_candidates_tokens;
            }
            
            // Calculate cost using configured pricing from config file
            let input_cost = (total_prompt_tokens as f32 * self.cost_per_1m_input_tokens) / 1_000_000.0;
            let output_cost = (total_candidates_tokens as f32 * self.cost_per_1m_output_tokens) / 1_000_000.0;
            let total_cost = input_cost + output_cost;
            
            let _ = event_bus.emit(Event::APICallCompleted {
                provider: "gemini".to_string(),
                tokens: total_tokens,
                cost: total_cost,
            }).await;
        }

        Ok(full_content)
    }
}
