use anyhow::{anyhow, Result};
use crate::llm_manager::LLMProvider;
use crate::event_bus::{Event, EventBus};
use log::{info};
use std::sync::Arc;
use tokio;
use ollama_rs::{Ollama, generation::completion::request::GenerationRequest, generation::options::GenerationOptions};
use futures::stream::StreamExt;
use async_trait::async_trait;

/// Ollama local LLM provider implementation
pub struct OllamaProvider {
    model: String,
    client: Ollama,
    max_tokens: usize,
    temperature: f32,
    event_bus: Option<Arc<EventBus>>,
}

impl OllamaProvider {
    /// Create a new Ollama provider with default settings
    pub fn new(
        model: Option<String>,
        temperature: Option<f32>,
        max_tokens: Option<usize>,
        event_bus: Option<Arc<EventBus>>,
    ) -> Result<Self> {
        let final_max_tokens = max_tokens.unwrap_or(128000);
        info!("OllamaProvider initialized with max_tokens: {}", final_max_tokens);
        
        Ok(Self {
            model: model.unwrap_or_else(|| "qwen3:8b".to_string()),
            client: Ollama::default(),
            max_tokens: final_max_tokens,
            temperature: temperature.unwrap_or(0.7),
            event_bus,
        })
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
        info!("Sending prompt to Ollama model '{}': {} characters", self.model, prompt.len());
        
        let mut request = GenerationRequest::new(self.model.clone(), prompt.to_string());
        
        // Set generation options including max_tokens and temperature
        let options = GenerationOptions::default()
            .num_predict(self.max_tokens as i32)
            .temperature(self.temperature);
        
        request = request.options(options);
        
        let mut stream = self.client.generate_stream(request).await
            .map_err(|e| anyhow!("Failed to start Ollama stream: {}", e))?;
        
        let mut full_response = String::new();
        let mut in_thinking = false;
        let mut thinking_buffer = String::new();
        let mut sent_thinking_length = 0;

        while let Some(chunk_result) = stream.next().await {
            let chunk_responses = chunk_result
                .map_err(|e| anyhow!("Error in stream chunk: {}", e))?;
            
            for chunk_response in chunk_responses {
                let content = &chunk_response.response;
                
                full_response.push_str(content);
                
                // Handle thinking tags (no direct printing - only send events)
                for part in content.split("<think>") {
                    if let Some(think_content) = part.strip_suffix("</think>") {
                        if !in_thinking {
                            thinking_buffer.clear();
                            sent_thinking_length = 0;
                        }
                        thinking_buffer.push_str(think_content);
                        
                        // Send complete reasoning trace (only new content)
                        if let Some(bus) = &self.event_bus {
                            let full_trace = thinking_buffer.trim().to_string();
                            if !full_trace.is_empty() {
                                let trace_to_send = if sent_thinking_length == 0 {
                                    format!("🤔 {} ✨", full_trace)
                                } else {
                                    format!("{} ✨", full_trace)
                                };
                                tokio::spawn({
                                    let bus = bus.clone();
                                    async move {
                                        let _ = bus.emit(Event::ReasoningTrace { message: trace_to_send }).await;
                                    }
                                });
                            }
                        }
                        
                        thinking_buffer.clear();
                        sent_thinking_length = 0;
                        in_thinking = false;
                    } else if in_thinking {
                        thinking_buffer.push_str(part);
                        
                        // Send new content periodically (only what's new since last send)
                        if let Some(bus) = &self.event_bus {
                            if thinking_buffer.len() > sent_thinking_length + 200 || 
                               (part.contains('.') || part.contains('!') || part.contains('?')) && thinking_buffer.len() > sent_thinking_length {
                                let new_content = &thinking_buffer[sent_thinking_length..];
                                let cleaned_new = new_content.trim().to_string();
                                if !cleaned_new.is_empty() {
                                    let trace_to_send = if sent_thinking_length == 0 {
                                        format!("🤔 {} ...", cleaned_new)
                                    } else {
                                        cleaned_new
                                    };
                                    sent_thinking_length = thinking_buffer.len();
                                    tokio::spawn({
                                        let bus = bus.clone();
                                        async move {
                                            let _ = bus.emit(Event::ReasoningTrace { message: trace_to_send }).await;
                                        }
                                    });
                                }
                            }
                        }
                    } else {
                        // Regular content outside thinking - just accumulate, don't print
                        if part.contains("<think>") {
                            in_thinking = true;
                            thinking_buffer.clear();
                            sent_thinking_length = 0;
                        }
                    }
                }
                
                // Check for incomplete thinking tags to set state
                if content.contains("<think>") && !content.contains("</think>") {
                    in_thinking = true;
                }
                
                // stdout().flush().await?;
            }
        }

        // Send any remaining buffered thinking content
        if !thinking_buffer.is_empty() {
            let new_content = &thinking_buffer[sent_thinking_length..];
            let cleaned_new = new_content.trim().to_string();
            if !cleaned_new.is_empty() {
                if let Some(bus) = &self.event_bus {
                    let trace_to_send = if sent_thinking_length == 0 {
                        format!("🤔 {} ...", cleaned_new)
                    } else {
                        cleaned_new
                    };
                    tokio::spawn({
                        let bus = bus.clone();
                        async move {
                            let _ = bus.emit(Event::ReasoningTrace { message: trace_to_send }).await;
                        }
                    });
                }
            }
        }

        // println!(); // Final newline
        info!("Ollama streaming complete. Response length: {}", full_response.len());

        if full_response.is_empty() {
            return Err(anyhow!("Empty response from Ollama"));
        }

        Ok(full_response)
    }
}
