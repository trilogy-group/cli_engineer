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
        // For now we simply echo the prompt as a placeholder.
        Ok(format!("Echo: {}", prompt))
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
