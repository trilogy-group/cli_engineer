use anyhow::{bail, Result};

use crate::llm_manager::LLMProvider;

pub struct Reviewer;

impl Reviewer {
    pub fn new() -> Self { Self }

    /// Review outputs for correctness.
    pub async fn review(&self, outputs: &[String], llm: &dyn LLMProvider) -> Result<()> {
        let joined = outputs.join("\n");
        let prompt = format!("Review the following outputs for correctness. Respond with 'ok' if they are correct.\\n{}", joined);
        let resp = llm.send_prompt(&prompt).await?;
        let lower = resp.to_lowercase();
        if lower.contains("error") || lower.contains("incorrect") {
            bail!("Reviewer found issues: {}", resp);
        }
        Ok(())
    }
}
