use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::interpreter::Task;
use crate::llm_manager::LLMProvider;

/// Represents a structured plan with categorized steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub goal: String,
    pub steps: Vec<Step>,
    pub dependencies: HashMap<String, Vec<String>>, // step_id -> dependent_step_ids
    pub estimated_complexity: ComplexityLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub description: String,
    pub category: StepCategory,
    pub inputs: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub success_criteria: Vec<String>,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepCategory {
    Analysis,        // Understanding requirements, analyzing code
    FileOperation,   // Creating, reading, updating files
    CodeGeneration,  // Writing new code
    CodeModification,// Modifying existing code
    Testing,         // Running tests or validation
    Documentation,   // Writing docs or comments
    Research,        // Looking up APIs, best practices
    Review,          // Code review and quality checks
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplexityLevel {
    Simple,   // 1-3 steps, straightforward changes
    Medium,   // 4-10 steps, moderate complexity
    Complex,  // 10+ steps or high interdependency
}

pub struct Planner {
    planning_prompt_template: String,
}

impl Planner {
    pub fn new() -> Self {
        Self {
            planning_prompt_template: Self::default_planning_prompt(),
        }
    }

    /// Create a structured plan for the given task using the provided LLM
    pub async fn plan(&self, task: &Task, llm: &dyn LLMProvider) -> Result<Plan> {
        let prompt = self.build_planning_prompt(task);
        let response = llm.send_prompt(&prompt).await
            .context("Failed to get planning response from LLM")?;
        
        // Parse the response into a structured plan
        self.parse_plan_response(&response, task)
            .context("Failed to parse plan from LLM response")
    }

    fn build_planning_prompt(&self, task: &Task) -> String {
        format!(
            "{}\n\nTask Description: {}\nGoal: {}\nContext: {}\nConstraints: {:?}",
            self.planning_prompt_template,
            task.description,
            task.goal,
            task.context,
            task.constraints
        )
    }

    fn parse_plan_response(&self, response: &str, task: &Task) -> Result<Plan> {
        // For now, use a simple parsing strategy
        // In a production system, this would use more sophisticated parsing
        // or ask the LLM to return structured JSON
        
        let lines: Vec<&str> = response.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        
        let mut steps = Vec::new();
        let mut current_step_lines = Vec::new();
        let mut step_counter = 1;
        
        for line in lines {
            if line.starts_with(|c: char| c.is_numeric()) && line.contains('.') {
                // This looks like a new step
                if !current_step_lines.is_empty() {
                    steps.push(self.create_step_from_lines(
                        &current_step_lines.join(" "),
                        step_counter - 1
                    ));
                    current_step_lines.clear();
                }
                // Remove the number prefix
                let step_text = line.splitn(2, '.').nth(1).unwrap_or(line).trim();
                current_step_lines.push(step_text);
                step_counter += 1;
            } else if !current_step_lines.is_empty() {
                // Continue the current step
                current_step_lines.push(line);
            }
        }
        
        // Don't forget the last step
        if !current_step_lines.is_empty() {
            steps.push(self.create_step_from_lines(
                &current_step_lines.join(" "),
                step_counter - 1
            ));
        }
        
        // If no structured steps were found, create a single step from the entire response
        if steps.is_empty() {
            steps.push(self.create_step_from_lines(response, 1));
        }
        
        // Determine complexity based on number of steps
        let complexity = match steps.len() {
            1..=3 => ComplexityLevel::Simple,
            4..=10 => ComplexityLevel::Medium,
            _ => ComplexityLevel::Complex,
        };
        
        Ok(Plan {
            goal: task.goal.clone(),
            steps,
            dependencies: HashMap::new(), // Could be enhanced to detect dependencies
            estimated_complexity: complexity,
        })
    }

    fn create_step_from_lines(&self, text: &str, index: usize) -> Step {
        // Categorize the step based on keywords
        let category = if text.contains("create") || text.contains("new file") {
            StepCategory::FileOperation
        } else if text.contains("write") || text.contains("implement") || text.contains("generate") {
            StepCategory::CodeGeneration
        } else if text.contains("modify") || text.contains("update") || text.contains("change") {
            StepCategory::CodeModification
        } else if text.contains("test") || text.contains("verify") || text.contains("validate") {
            StepCategory::Testing
        } else if text.contains("document") || text.contains("comment") {
            StepCategory::Documentation
        } else if text.contains("analyze") || text.contains("understand") || text.contains("examine") {
            StepCategory::Analysis
        } else if text.contains("research") || text.contains("look up") || text.contains("find") {
            StepCategory::Research
        } else if text.contains("review") || text.contains("check") {
            StepCategory::Review
        } else {
            StepCategory::Analysis // Default
        };
        
        Step {
            id: format!("step_{}", index),
            description: text.to_string(),
            category,
            inputs: Vec::new(),
            expected_outputs: Vec::new(),
            success_criteria: vec![format!("Successfully complete: {}", text)],
            estimated_tokens: text.len() / 4, // Rough estimate
        }
    }

    fn default_planning_prompt() -> String {
        r#"You are a senior software architect creating a detailed implementation plan.
        
Given the task below, create a numbered list of specific, actionable steps.
Each step should be:
- Clear and specific about what needs to be done
- Focused on a single action or outcome
- Ordered logically with dependencies in mind

Consider:
- What files need to be created or modified
- What code needs to be written or changed
- What tests or validations are needed
- What documentation should be updated

Provide the steps as a numbered list (1., 2., 3., etc.)"#.to_string()
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}
