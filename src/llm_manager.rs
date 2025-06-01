use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use crate::event_bus::{EventBus, Event, EventEmitter};
use crate::impl_event_emitter;

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
}

/// Dummy provider used when no remote LLM is available.
pub struct LocalProvider;

#[async_trait]
impl LLMProvider for LocalProvider {
    fn name(&self) -> &str { "local" }

    fn context_size(&self) -> usize { 4096 }

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
}

/// Manager that keeps track of multiple providers and context limits.
pub struct LLMManager {
    providers: Vec<Box<dyn LLMProvider>>,
    event_bus: Option<Arc<EventBus>>,
}

impl LLMManager {
    /// Create a new manager with the given providers.
    pub fn new(providers: Vec<Box<dyn LLMProvider>>, event_bus: Arc<EventBus>) -> Self {
        Self {
            providers,
            event_bus: Some(event_bus),
        }
    }

    /// Get the active provider.
    pub fn provider(&self) -> &dyn LLMProvider {
        &*self.providers[0]
    }

    pub async fn send_prompt(&self, prompt: &str) -> Result<String> {
        // For now, use first provider
        let provider = &self.providers[0];
        
        // Emit API call started event
        if let Some(bus) = &self.event_bus {
            bus.emit(Event::APICallStarted {
                provider: provider.name().to_string(),
                model: provider.model_name().to_string(),
            }).await?;
        }
        
        // Send prompt
        let result = provider.send_prompt(prompt).await;
        
        // Emit completion or error event
        if let Some(bus) = &self.event_bus {
            match &result {
                Ok(response) => {
                    // In real implementation, we'd calculate actual tokens and cost
                    bus.emit(Event::APICallCompleted {
                        provider: provider.name().to_string(),
                        tokens: response.len(), // Placeholder
                        cost: 0.0, // Placeholder
                    }).await?;
                }
                Err(e) => {
                    bus.emit(Event::APIError {
                        provider: provider.name().to_string(),
                        error: e.to_string(),
                    }).await?;
                }
            }
        }
        
        result
    }
}

// Implement EventEmitter trait for LLMManager
impl_event_emitter!(LLMManager);
