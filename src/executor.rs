use anyhow::Result;

use crate::llm_manager::LLMProvider;
use crate::planner::Plan;

/// Executes planned steps using a coding LLM.
pub struct Executor;

impl Executor {
    pub fn new() -> Self { Self }

    /// Execute each step and collect the responses.
    pub async fn execute(&self, plan: &Plan, llm: &dyn LLMProvider) -> Result<Vec<String>> {
        let mut outputs = Vec::new();
        for step in &plan.steps {
            let prompt = format!("Execute step: {}", step);
            let resp = llm.send_prompt(&prompt).await?;
            outputs.push(resp);
        }
        Ok(outputs)
    }
}
