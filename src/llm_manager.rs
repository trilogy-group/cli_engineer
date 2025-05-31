use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

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

const MAX_RETRIES: usize = 5;

async fn send_with_backoff<F, Fut>(mut f: F) -> Result<String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response>>,
{
    let mut delay = Duration::from_secs(1);
    for _ in 0..MAX_RETRIES {
        match f().await {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                if !resp.status().is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow!(text));
                }
                return Ok(resp.text().await?);
            }
            Err(err) => {
                tokio::time::sleep(delay).await;
                delay *= 2;
                if delay.as_secs() > 32 {
                    return Err(err.into());
                }
            }
        }
    }
    Err(anyhow!("exceeded retries"))
}

/// Provider using the OpenAI API.
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    context: usize,
}

impl OpenAIProvider {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY"),
            model: model.to_string(),
            context: 8192,
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    fn name(&self) -> &str { "openai" }
    fn context_size(&self) -> usize { self.context }
    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let payload = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0
        });
        let api = "https://api.openai.com/v1/chat/completions".to_string();
        send_with_backoff(|| async {
            self.client.post(&api)
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await
        }).await.and_then(|text| {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        })
    }
}

/// Provider using Anthropic models.
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    context: usize,
}

impl AnthropicProvider {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY"),
            model: model.to_string(),
            context: 200000,
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }
    fn context_size(&self) -> usize { self.context }
    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let payload = json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}]
        });
        let api = "https://api.anthropic.com/v1/messages".to_string();
        send_with_backoff(|| async {
            self.client.post(&api)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&payload)
                .send()
                .await
        }).await.and_then(|text| {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            Ok(v["content"][0]["text"].as_str().unwrap_or("").to_string())
        })
    }
}

/// Provider using OpenRouter with an OpenAI-compatible API.
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    model: String,
    context: usize,
}

impl OpenRouterProvider {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY"),
            model: model.to_string(),
            context: 8192,
        }
    }
}

#[async_trait]
impl LLMProvider for OpenRouterProvider {
    fn name(&self) -> &str { "openrouter" }
    fn context_size(&self) -> usize { self.context }
    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let payload = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0
        });
        let api = "https://openrouter.ai/api/v1/chat/completions".to_string();
        send_with_backoff(|| async {
            self.client.post(&api)
                .header("Authorization", format!("Bearer {}", &self.api_key))
                .json(&payload)
                .send()
                .await
        }).await.and_then(|text| {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        })
    }
}

/// Provider using xAI's Grok model.
pub struct XAIProvider {
    client: Client,
    api_key: String,
    model: String,
    context: usize,
}

impl XAIProvider {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: std::env::var("XAI_API_KEY").expect("XAI_API_KEY"),
            model: model.to_string(),
            context: 8192,
        }
    }
}

#[async_trait]
impl LLMProvider for XAIProvider {
    fn name(&self) -> &str { "xai" }
    fn context_size(&self) -> usize { self.context }
    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let payload = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.0
        });
        let api = "https://api.x.ai/v1/chat/completions".to_string();
        send_with_backoff(|| async {
            self.client.post(&api)
                .header("Authorization", format!("Bearer {}", &self.api_key))
                .json(&payload)
                .send()
                .await
        }).await.and_then(|text| {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        })
    }
}

/// Provider using a local ollama server.
pub struct OllamaProvider {
    client: Client,
    model: String,
    context: usize,
}

impl OllamaProvider {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            model: model.to_string(),
            context: 8192,
        }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn name(&self) -> &str { "ollama" }
    fn context_size(&self) -> usize { self.context }
    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let payload = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        });
        let api = "http://localhost:11434/api/chat".to_string();
        send_with_backoff(|| async {
            self.client.post(&api)
                .json(&payload)
                .send()
                .await
        }).await.and_then(|text| {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            Ok(v["message"]["content"].as_str().unwrap_or("").to_string())
        })
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

    /// Create a manager based on environment variables.
    pub fn from_env() -> Self {
        let provider_name = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "openai".into());
        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4-turbo".into());
        let provider: Box<dyn LLMProvider> = match provider_name.as_str() {
            "anthropic" => Box::new(AnthropicProvider::new(&model)),
            "openrouter" => Box::new(OpenRouterProvider::new(&model)),
            "xai" => Box::new(XAIProvider::new(&model)),
            "ollama" => Box::new(OllamaProvider::new(&model)),
            _ => Box::new(OpenAIProvider::new(&model)),
        };
        Self::new(vec![provider])
    }

    /// Get the active provider.
    pub fn provider(&self) -> &dyn LLMProvider {
        &*self.providers[self.active]
    }
}
