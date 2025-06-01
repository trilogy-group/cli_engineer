
use anyhow::{Result, Context, anyhow};
use async_trait::async_trait;
use reqwest;
use serde_json;
use std::env;

use crate::llm_manager::LLMProvider;

#[derive(Debug, Clone)]
pub struct OpenRouterProvider {
    pub model: String,
    pub temperature: f32,
    api_key: String,
    client: reqwest::Client,
}

impl OpenRouterProvider {
    pub fn new(model: Option<String>, temperature: Option<f32>) -> Result<Self> {
        let api_key = env::var("OPENROUTER_API_KEY")
            .context("OPENROUTER_API_KEY environment variable not set")?;
        Ok(Self {
            model: model.unwrap_or_else(|| "deepseek/deepseek-r1-0528-qwen3-8b".to_string()),
            temperature: temperature.unwrap_or(0.2),
            api_key,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl LLMProvider for OpenRouterProvider {
    fn name(&self) -> &str { "openrouter" }
    fn context_size(&self) -> usize { 32768 } // OpenRouter supports large context windows for some models
    fn model_name(&self) -> &str { &self.model }

    async fn send_prompt(&self, prompt: &str) -> Result<String> {
        let url = "https://openrouter.ai/api/v1/chat/completions";
        let req_body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": self.temperature,
        });
        let resp = self.client
            .post(url)
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://github.com/trilogy-group/cli_engineer")
            .header("X-Title", "cli_engineer")
            .json(&req_body)
            .send()
            .await
            .context("Failed to send request to OpenRouter")?;
        if !resp.status().is_success() {
            return Err(anyhow!("OpenRouter API error: {}", resp.status()));
        }
        let json: serde_json::Value = resp.json().await.context("Failed to parse OpenRouter response")?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("No content in OpenRouter response"))?;
        Ok(content.to_string())
    }
}
