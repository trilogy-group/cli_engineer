use crate::config::Config;
use crate::event_bus::{Event, EventBus, EventEmitter};
use crate::impl_event_emitter;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait representing an LLM provider.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Name of the provider.
    fn name(&self) -> &str;

    /// Maximum context size in tokens.
    #[allow(dead_code)]
    fn context_size(&self) -> usize;

    /// Send a prompt to the provider and return the response.
    async fn send_prompt(&self, prompt: &str) -> Result<String>;

    /// Model name of the provider.
    fn model_name(&self) -> &str {
        "Unknown"
    }

    /// Whether this provider handles its own metrics and cost tracking.
    /// If true, LLMManager will not emit duplicate APICallCompleted events.
    fn handles_own_metrics(&self) -> bool {
        false
    }
}

/// Dummy provider used when no remote LLM is available.
pub struct LocalProvider;

#[async_trait]
impl LLMProvider for LocalProvider {
    fn name(&self) -> &str {
        "local"
    }

    fn context_size(&self) -> usize {
        4096
    }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        if let Some(task) = prompt.strip_prefix("Plan the following task:") {
            let mut steps = Vec::new();
            for (i, part) in task.split('.').enumerate() {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    steps.push(format!("{}. {}", i + 1, trimmed));
                }
            }
            if steps.is_empty() {
                Ok("1. No steps generated".to_string())
            } else {
                Ok(steps.join("\n"))
            }
        } else if let Some(step) = prompt.strip_prefix("Execute step:") {
            Ok(format!("Executed: {}", step.trim()))
        } else if prompt.starts_with("Review") {
            Ok("All good".to_string())
        } else {
            Ok(prompt.to_string())
        }
    }

    fn handles_own_metrics(&self) -> bool {
        false
    }
}

/// Manager that keeps track of multiple providers and context limits.
pub struct LLMManager {
    providers: Vec<Box<dyn LLMProvider>>,
    event_bus: Option<Arc<EventBus>>,
    config: Option<Arc<Config>>,
}

impl LLMManager {
    /// Create a new manager with the given providers.
    pub fn new(
        providers: Vec<Box<dyn LLMProvider>>,
        event_bus: Arc<EventBus>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            providers,
            event_bus: Some(event_bus),
            config: Some(config),
        }
    }

    /// Get the active provider.
    #[allow(dead_code)]
    pub fn provider(&self) -> &dyn LLMProvider {
        &*self.providers[0]
    }

    /// Get the context size of the active provider.
    pub fn get_context_size(&self) -> usize {
        if self.providers.is_empty() {
            4096 // Default fallback
        } else {
            self.providers[0].context_size()
        }
    }

    /// Send a prompt to the first available provider.
    pub async fn send_prompt(&self, prompt: &str) -> anyhow::Result<String> {
        if self.providers.is_empty() {
            return Err(anyhow::anyhow!("No providers available"));
        }

        let provider = &self.providers[0];

        // Emit API call started event
        if let Some(bus) = &self.event_bus {
            let _ = bus
                .emit(Event::APICallStarted {
                    provider: provider.name().to_string(),
                    model: provider.model_name().to_string(),
                })
                .await;
        }

        // Send prompt
        let result = provider.send_prompt(prompt).await;

        // Emit completion or error event
        if let Some(bus) = &self.event_bus {
            match &result {
                Ok(response) => {
                    if !provider.handles_own_metrics() {
                        // Calculate approximate token counts (rough estimate: 1 token ≈ 4 characters)
                        let input_tokens = prompt.len() / 4;
                        let output_tokens = response.len() / 4;
                        let total_tokens = input_tokens + output_tokens;

                        // Calculate cost based on model configuration
                        let cost = self.calculate_cost(provider.name(), input_tokens, output_tokens);

                        let _ = bus
                            .emit(Event::APICallCompleted {
                                provider: provider.name().to_string(),
                                tokens: total_tokens,
                                cost,
                            })
                            .await;
                    }
                }
                Err(e) => {
                    let _ = bus
                        .emit(Event::APIError {
                            provider: provider.name().to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
            }
        }

        result
    }

    /// Calculate cost for API call based on provider configuration
    fn calculate_cost(
        &self,
        provider_name: &str,
        input_tokens: usize,
        output_tokens: usize,
    ) -> f32 {
        if let Some(config) = &self.config {
            let provider_config = match provider_name.to_lowercase().as_str() {
                "openai" => &config.ai_providers.openai,
                "anthropic" => &config.ai_providers.anthropic,
                "openrouter" => &config.ai_providers.openrouter,
                "gemini" => &config.ai_providers.gemini,
                _ => return 0.0,
            };

            if let Some(provider_config) = provider_config {
                let input_cost = provider_config.cost_per_1m_input_tokens.unwrap_or(0.0)
                    * (input_tokens as f32)
                    / 1_000_000.0;
                let output_cost = provider_config.cost_per_1m_output_tokens.unwrap_or(0.0)
                    * (output_tokens as f32)
                    / 1_000_000.0;
                return input_cost + output_cost;
            }
        }
        0.0
    }
}

// Implement EventEmitter trait for LLMManager
impl_event_emitter!(LLMManager);
