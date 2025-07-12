use crate::{
    config::Config, interpreter::Task, iteration_context::IterationContext, llm_manager::LLMManager,
    tool_manager::ToolManager,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    Analysis,         // Understanding requirements, analyzing code
    FileOperation,    // Creating, reading, updating files
    CodeGeneration,   // Writing new code
    CodeModification, // Modifying existing code
    Testing,          // Running tests or validation
    Documentation,    // Writing docs or comments
    Research,         // Looking up APIs, best practices
    Review,           // Code review and quality checks
    CommandExecution, // Running allowlisted shell commands
    ToolInvocation,   // Invoking MCP (Model Context Protocol) tools
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplexityLevel {
    Simple,  // 1-3 steps, straightforward changes
    Medium,  // 4-10 steps, moderate complexity
    Complex, // 10+ steps or high interdependency
}

pub struct Planner {}

impl Planner {
    pub fn new() -> Self {
        Self {}
    }

    /// Create a structured plan for the given task using the provided LLM
    pub async fn plan(
        &self,
        task: &Task,
        llm_manager: &LLMManager,
        config: Option<&Config>,
        iteration_context: Option<&IterationContext>,
        tool_manager: Option<&ToolManager>,
    ) -> Result<Plan> {
        let prompt = self.build_planning_prompt(task, config, iteration_context, tool_manager);
        let response = llm_manager
            .send_prompt(&prompt)
            .await
            .context("Failed to get planning response from LLM")?;

        // Parse the response into a structured plan
        self.parse_plan_response(&response, task)
            .context("Failed to parse plan from LLM response")
    }

    fn build_planning_prompt(
        &self,
        task: &Task,
        config: Option<&Config>,
        iteration_context: Option<&IterationContext>,
        tool_manager: Option<&ToolManager>,
    ) -> String {
        let mut prompt = format!(
            "You are an expert software architect creating a step-by-step plan.

Task: {}
Goal: {}

Create a detailed, actionable plan with specific steps. Each step should:
1. Have a clear, specific action
2. Build upon previous steps
3. Be categorized appropriately

IMPORTANT: Base your plan ONLY on the actual task requirements and existing code. DO NOT:
- Invent problems that don't exist
- Add unnecessary security checks for simple scripts
- Create steps to fix non-existent issues
- Add complex error handling for trivial programs

Categories available:
- File Operation: Create, read, update, delete files
- Code Generation: Generate new code from scratch
- Code Modification: Modify existing code (use for files that already exist)
- Testing: Create tests (DO NOT execute them)
- Documentation: Create necessary documentation
- Research: Research information or requirements
- Review: Review existing code/documentation
- Command Execution: Execute shell commands for compilation, testing, and analysis

Provide the plan as a numbered list. Be concise and specific.",
            task.description, task.goal
        );

        // Add git-related instructions if disable_auto_git is enabled
        if let Some(cfg) = config {
            if cfg.execution.disable_auto_git {
                prompt.push_str("\n\nIMPORTANT: Do NOT include git repository initialization (git init) or git-related setup steps unless explicitly requested in the task description. Focus only on the core functionality requested.");
            }
            
            // Add command execution capabilities if enabled
            if cfg.execution.enable_code_execution {
                let allowed_commands = cfg.execution.allowed_commands.iter()
                    .map(|c| format!("   - {}", c))
                    .collect::<Vec<_>>()
                    .join("\n");
                prompt.push_str(&format!(
                    "\n\nCOMMAND EXECUTION AVAILABLE:\nYou MUST include 'Command Execution' steps in your plan for verification. This is MANDATORY, not optional.\n\nMANDATORY VERIFICATION REQUIREMENTS:\n1. ALWAYS include 'Command Execution: Run cargo build' IMMEDIATELY after:\n   - Creating or modifying Cargo.toml (to verify dependency versions exist)\n   - Creating or modifying any Rust source code\n   - Any step that changes project dependencies\n\n2. ALWAYS include 'Command Execution: Run cargo test' IMMEDIATELY after:\n   - Creating test files\n   - Modifying existing test files\n   - Any changes that might affect test behavior\n\n3. ALWAYS include 'Command Execution: Run [language] [script]' IMMEDIATELY after:\n   - Creating Python, Node.js, or other executable scripts\n   - Modifying configuration that affects runtime behavior\n\n4. SYSTEMATIC VERIFICATION PATTERN:\n   Code Change → IMMEDIATE Verification → Next Step\n   Never skip verification steps between code changes!\n\nAllowed command prefixes:\n{}\n\nMANDATORY VERIFICATION EXAMPLES:\n- Step 3: Create Cargo.toml with dependencies\n- Step 4: VERIFY → Command Execution: Run 'cargo build' to verify dependencies exist and compile\n- Step 5: Create src/main.rs with core logic\n- Step 6: VERIFY → Command Execution: Run 'cargo build' to verify code compiles\n- Step 7: Create tests/integration.rs\n- Step 8: VERIFY → Command Execution: Run 'cargo test' to verify tests pass\n\nCRITICAL: Never declare a task complete without verification steps. Verification failures must be caught during development, not after deployment!",
                    allowed_commands
                ));
            }
        }
        
        // Add available MCP tools if tool_manager is provided
        if let Some(tm) = tool_manager {
            let tools = tm.list_tools();
            if !tools.is_empty() {
                prompt.push_str("\n\nAVAILABLE MCP TOOLS:");
                prompt.push_str("\nYou can include 'Tool Invocation' steps to use external tools:");
                prompt.push_str("\n\nAvailable tools:");
                for tool in tools {
                    prompt.push_str(&format!("\n- {}: {}", tool.name, tool.description.as_deref().unwrap_or("No description")));
                    if let Some(params) = &tool.parameters {
                        prompt.push_str(&format!("\n  Parameters: {}", serde_json::to_string(params).unwrap_or_default()));
                    }
                }
                prompt.push_str("\n\nExample tool invocation steps:");
                prompt.push_str("\n- Step N: Tool Invocation: Use 'tool_name' with {input: 'value'} to perform task");
                prompt.push_str("\n- Step N: Tool Invocation: Call 'another_tool' to analyze results");
                prompt.push_str("\n\nTool invocations can be used for:");
                prompt.push_str("\n- Accessing external data sources");
                prompt.push_str("\n- Performing specialized operations");
                prompt.push_str("\n- Integrating with external services");
            }
        }

        // Add iteration context if provided
        if let Some(ctx) = iteration_context {
            prompt.push_str(&format!("\n\nIteration Context:\n{}", ctx));

            // Add specific instructions for handling existing files
            if ctx.has_existing_files() {
                prompt.push_str(
                    "\n\nIMPORTANT: Files already exist from previous iterations. When planning:",
                );
                prompt.push_str("\n1. DO NOT recreate files that already exist - use 'Code Modification' steps instead");
                prompt.push_str(
                    "\n2. Focus on addressing the specific issues identified in the review",
                );
                prompt.push_str("\n3. If a file needs changes, describe what needs to be modified, not recreated");
                prompt.push_str("\n4. Only create new files if they don't already exist");
            }
            
            // Add adaptive planning based on command failures
            if ctx.has_failed_builds() || ctx.has_failed_tests() {
                prompt.push_str("\n\n🚨 CRITICAL: Previous command executions have FAILED!");
                
                if ctx.has_failed_builds() {
                    prompt.push_str("\n\nBUILD FAILURES DETECTED:");
                    prompt.push_str("\n1. Your FIRST priority is to fix build errors");
                    prompt.push_str("\n2. Examine the error messages in the failed commands section");
                    prompt.push_str("\n3. Plan steps to fix dependencies, syntax errors, or configuration issues");
                    prompt.push_str("\n4. Include 'Command Execution: Run [build command]' immediately after fixes");
                }
                
                if ctx.has_failed_tests() {
                    prompt.push_str("\n\nTEST FAILURES DETECTED:");
                    prompt.push_str("\n1. After fixing builds, focus on fixing test failures");
                    prompt.push_str("\n2. Review test output to understand what's failing");
                    prompt.push_str("\n3. Plan steps to fix the failing tests or implementation");
                    prompt.push_str("\n4. Include 'Command Execution: Run [test command]' after fixes");
                }
                
                prompt.push_str("\n\nDO NOT proceed with new features until all builds and tests pass!");
            }
            
            // Instructions for using command outputs
            if !ctx.command_outputs.is_empty() {
                prompt.push_str("\n\nCOMMAND OUTPUT CONTEXT:");
                prompt.push_str("\nPrevious command executions have been recorded. Use their output to:");
                prompt.push_str("\n1. Understand what errors need to be fixed");
                prompt.push_str("\n2. Verify what has already been successfully completed");
                prompt.push_str("\n3. Avoid repeating successful steps");
                prompt.push_str("\n4. Focus on addressing failures and warnings");
            }
        }

        prompt
    }

    fn parse_plan_response(&self, response: &str, task: &Task) -> Result<Plan> {
        // For now, use a simple parsing strategy
        // In a production system, this would use more sophisticated parsing
        // or ask the LLM to return structured JSON

        let lines: Vec<&str> = response
            .lines()
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
                    steps.push(
                        self.create_step_from_lines(
                            &current_step_lines.join(" "),
                            step_counter - 1,
                        ),
                    );
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
            steps
                .push(self.create_step_from_lines(&current_step_lines.join(" "), step_counter - 1));
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
        } else if text.contains("write") || text.contains("implement") || text.contains("generate")
        {
            StepCategory::CodeGeneration
        } else if text.contains("modify") || text.contains("update") || text.contains("change") {
            StepCategory::CodeModification
        } else if text.contains("test") || text.contains("verify") || text.contains("validate") {
            StepCategory::Testing
        } else if text.contains("document") || text.contains("comment") {
            StepCategory::Documentation
        } else if text.contains("analyze")
            || text.contains("understand")
            || text.contains("examine")
        {
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
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}
