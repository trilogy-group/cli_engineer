use anyhow::Result;
use async_trait::async_trait;

/// Trait representing an LLM provider.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Name of the provider.
    fn name(&self) -> &str;

    /// Maximum context size in tokens.
    fn context_size(&self) -> usize;

    /// Send a prompt to the provider and return the response.
    async fn send_prompt(&self, prompt: &str) -> Result<String>;
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
    active: usize,
}

impl LLMManager {
    /// Create a new manager with the given providers.
    pub fn new(providers: Vec<Box<dyn LLMProvider>>) -> Self {
        Self { providers, active: 0 }
    }

    /// Get the active provider.
    pub fn provider(&self) -> &dyn LLMProvider {
        &*self.providers[self.active]
    }
}
