use anyhow::Result;

use crate::llm_manager::LLMProvider;

pub struct Reviewer;

impl Reviewer {
    pub fn new() -> Self { Self }

    /// Review outputs for correctness.
    pub async fn review(&self, outputs: &[String], llm: &dyn LLMProvider) -> Result<()> {
        let joined = outputs.join("\n");
        let prompt = format!("Review the following outputs:\n{}", joined);
        let _resp = llm.send_prompt(&prompt).await?;
        // Placeholder: In a real implementation the response would influence further actions.
        Ok(())
    }
}
