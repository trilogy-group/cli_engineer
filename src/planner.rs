use anyhow::Result;

use crate::interpreter::Task;
use crate::llm_manager::LLMProvider;

/// Represents a sequence of steps to perform.
#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<String>,
}

pub struct Planner;

impl Planner {
    pub fn new() -> Self { Self }

    /// Create a plan for the given task using the provided LLM.
    pub async fn plan(&self, task: &Task, llm: &dyn LLMProvider) -> Result<Plan> {
        let prompt = format!("Plan the following task: {}", task.description);
        let resp = llm.send_prompt(&prompt).await?;
        // Very naive parsing: split lines as steps.
        let steps = resp.lines().map(|l| l.trim().to_string()).collect();
        Ok(Plan { steps })
    }
}
