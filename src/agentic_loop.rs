use anyhow::Result;
use log::{info, error};

use crate::interpreter::Interpreter;
use crate::planner::{Planner, Plan};
use crate::executor::Executor;
use crate::reviewer::Reviewer;
use crate::llm_manager::LLMManager;

/// Controls the iterative planning-action-review cycle.
pub struct AgenticLoop<'a> {
    interpreter: Interpreter,
    planner: Planner,
    executor: Executor,
    reviewer: Reviewer,
    llm_manager: &'a LLMManager,
    max_iterations: usize,
}

impl<'a> AgenticLoop<'a> {
    pub fn new(llm_manager: &'a LLMManager, max_iterations: usize) -> Self {
        Self {
            interpreter: Interpreter::new(),
            planner: Planner::new(),
            executor: Executor::new(),
            reviewer: Reviewer::new(),
            llm_manager,
            max_iterations,
        }
    }

    /// Run the agentic loop on the given input.
    pub async fn run(&self, input: &str) -> Result<()> {
        let task = self.interpreter.interpret(input)?;
        info!("Interpreted task: {}", task.description);
        let provider = self.llm_manager.provider();
        let mut iteration = 0;
        loop {
            if iteration >= self.max_iterations {
                info!("Reached max iterations");
                break;
            }
            iteration += 1;
            info!("Planning iteration {}", iteration);
            let plan: Plan = self.planner.plan(&task, provider).await?;
            info!("Plan: {:?}", plan.steps);
            let outputs = self.executor.execute(&plan, provider).await?;
            info!("Execution complete");
            if let Err(err) = self.reviewer.review(&outputs, provider).await {
                error!("Review failed: {}", err);
            }
            // For now exit after one iteration.
            break;
        }
        Ok(())
    }
}
