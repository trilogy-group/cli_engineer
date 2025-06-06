use anyhow::{Context as AnyhowContext, Result};
use std::collections::HashMap;
use std::sync::Arc;

use crate::artifact::{ArtifactManager, ArtifactType};
use crate::context::ContextManager;
use crate::event_bus::{Event, EventBus};
use crate::llm_manager::LLMManager;
use crate::planner::{Plan, Step, StepCategory};
use log::{info, warn};
use crate::CommandKind;

/// Result of executing a single step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: String,
    pub artifacts_created: Vec<String>,
    #[allow(dead_code)]
    pub tokens_used: usize,
    pub error: Option<String>,
}

/// Executes planned steps using a coding LLM
pub struct Executor {
    artifact_manager: Option<Arc<ArtifactManager>>,
    context_manager: Option<Arc<ContextManager>>,
    event_bus: Option<Arc<EventBus>>,
    llm_manager: Arc<LLMManager>,
    command: Option<CommandKind>,
}

impl Executor {
    pub fn new(llm_manager: Arc<LLMManager>) -> Self {
        Self {
            artifact_manager: None,
            context_manager: None,
            event_bus: None,
            llm_manager,
            command: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_artifact_manager(mut self, manager: Arc<ArtifactManager>) -> Self {
        self.artifact_manager = Some(manager);
        self
    }

    #[allow(dead_code)]
    pub fn with_context_manager(mut self, manager: Arc<ContextManager>) -> Self {
        self.context_manager = Some(manager);
        self
    }

    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn with_command(mut self, command: CommandKind) -> Self {
        self.command = Some(command);
        self
    }

    /// Execute the entire plan and return results for each step
    pub async fn execute(&self, plan: &Plan, context_id: &str) -> Result<Vec<StepResult>> {
        let mut results = Vec::new();

        // Emit plan execution started event
        if let Some(bus) = &self.event_bus {
            let _ = bus
                .emit(Event::Custom {
                    event_type: "plan_execution_started".to_string(),
                    data: serde_json::json!({
                        "plan_goal": plan.goal,
                        "total_steps": plan.steps.len(),
                        "complexity": format!("{:?}", plan.estimated_complexity)
                    }),
                })
                .await;
        }

        for (index, step) in plan.steps.iter().enumerate() {
            // Check dependencies (if implemented)
            if !self.dependencies_met(&step.id, &plan.dependencies, &results) {
                results.push(StepResult {
                    step_id: step.id.clone(),
                    success: false,
                    output: String::new(),
                    artifacts_created: Vec::new(),
                    tokens_used: 0,
                    error: Some("Dependencies not met".to_string()),
                });
                continue;
            }

            // Execute the step
            let result = self
                .execute_step(step, context_id, index + 1, plan.steps.len())
                .await
                .context(format!("Failed to execute step: {}", step.description))?;

            // Emit step completed event
            if let Some(bus) = &self.event_bus {
                let _ = bus
                    .emit(Event::TaskProgress {
                        task_id: step.id.clone(),
                        progress: ((index + 1) as f32 / plan.steps.len() as f32) * 100.0,
                        message: format!(
                            "Completed step {}/{}: {}",
                            index + 1,
                            plan.steps.len(),
                            step.description
                        ),
                    })
                    .await;
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Execute a single step based on its category
    async fn execute_step(
        &self,
        step: &Step,
        context_id: &str,
        step_num: usize,
        total_steps: usize,
    ) -> Result<StepResult> {
        info!(
            "Executing step {}/{}: {}",
            step_num, total_steps, step.description
        );

        // Build the appropriate prompt based on step category
        let base_prompt = self.build_step_prompt(step, step_num, total_steps);

        // Get all context messages if available
        let full_prompt = if let Some(ctx_mgr) = &self.context_manager {
            // First add the step description to context
            ctx_mgr
                .add_message(
                    context_id,
                    "user".to_string(),
                    format!("Step {}: {}", step_num, step.description),
                )
                .await?;

            // Get all messages from context (including codebase files)
            let messages = ctx_mgr.get_messages(context_id, None).await?;
            
            // Build a complete prompt including context
            let mut context_prompt = String::new();
            
            // Add system messages (codebase files) first
            let mut _system_msg_count = 0;
            for msg in &messages {
                if msg.role == "system" {
                    context_prompt.push_str(&msg.content);
                    context_prompt.push_str("\n\n");
                    _system_msg_count += 1;
                }
            }
            
            // Add the actual step prompt
            context_prompt.push_str(&base_prompt);
            
            context_prompt
        } else {
            info!("No context manager available - using standalone prompt");
            base_prompt
        };

        // Send to LLM
        let response = self.llm_manager.send_prompt(&full_prompt).await?;

        info!("Received response from LLM for step {}", step_num);

        // Debug: log the response for CodeModification steps
        if matches!(step.category, StepCategory::CodeModification) {
            let preview = if response.len() > 200 {
                format!("{}...", &response[..200])
            } else {
                response.clone()
            };
            info!("CodeModification response preview: {}", preview);
        }

        // Add response to context
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr
                .add_message(context_id, "assistant".to_string(), response.clone())
                .await?;
        }

        // Process the response based on category
        let mut result = StepResult {
            step_id: step.id.clone(),
            success: true,
            output: response.clone(),
            artifacts_created: Vec::new(),
            tokens_used: 0,
            error: None,
        };

        // Handle category-specific post-processing
        match step.category {
            StepCategory::FileOperation
            | StepCategory::CodeGeneration
            | StepCategory::CodeModification
            | StepCategory::Testing
            | StepCategory::Documentation => {
                // Try to extract and save code artifacts
                if let Some(artifact_mgr) = &self.artifact_manager {
                    let artifacts = self
                        .extract_code_artifacts(&response, &step.description, &step.category)
                        .await?;
                    for (filename, content) in artifacts {
                        // Safety check: For Docs command, only allow files in docs/ directory
                        if matches!(self.command, Some(CommandKind::Docs)) {
                            if !filename.starts_with("docs/") {
                                warn!(
                                    "Refusing to create '{}' during Docs command - only files in docs/ directory are allowed",
                                    filename
                                );
                                continue;
                            }
                        }
                        
                        let extension = filename.split('.').last();
                        let artifact_type = match extension {
                            Some("rs") => ArtifactType::SourceCode,
                            Some("toml") => ArtifactType::Configuration,
                            Some("json") => ArtifactType::Configuration,
                            Some("md") => ArtifactType::Documentation,
                            Some("txt") => ArtifactType::Documentation,
                            Some("sh") => ArtifactType::Script,
                            Some("py") => ArtifactType::SourceCode,
                            Some("js") => ArtifactType::SourceCode,
                            _ => ArtifactType::Other("unknown".to_string()),
                        };
                        let mut metadata = HashMap::new();
                        metadata.insert("step_id".to_string(), step.id.clone());
                        metadata.insert("category".to_string(), format!("{:?}", step.category));

                        match artifact_mgr
                            .create_artifact(
                                filename.clone(),
                                artifact_type,
                                content.clone(),
                                metadata,
                            )
                            .await
                        {
                            Ok(artifact) => {
                                result.artifacts_created.push(artifact.id);
                            }
                            Err(e) => {
                                eprintln!("Failed to create artifact {}: {}", filename, e);
                            }
                        }
                    }
                }
            }
            _ => {
                // Other categories don't typically create artifacts
            }
        }

        Ok(result)
    }

    fn build_step_prompt(&self, step: &Step, step_num: usize, total_steps: usize) -> String {
        let category_context = match step.category {
            StepCategory::Analysis => {
                "\n\nANALYSIS RULES:
1. Provide analysis in text format only
2. DO NOT create any files
3. Include findings, code analysis, and recommendations in your response:"
            }
            StepCategory::FileOperation => {
                "Create or modify the specified file. When providing code, use XML artifact format below. Provide the COMPLETE file content:"
            }
            StepCategory::CodeGeneration => {
                "Generate the requested code. When providing code, use XML artifact format below. Provide COMPLETE, working code:"
            }
            StepCategory::CodeModification => {
                "Modify the existing code as requested. 

YOU MUST use XML artifact format below. Here's EXACTLY what to output:

<artifact filename=\"filename.ext\" type=\"language\">
<![CDATA[
entire file content here (including any markdown code blocks if this is a .md file)
]]>
</artifact>

RULES:
1. ALWAYS start with <artifact> (NO filename after artifact)
2. Use filename=\"filename.ext\" and type=\"language\" headers
3. Use <![CDATA[ and ]]> to enclose the file content
4. Lines starting with - are removed
5. Lines starting with + are added
6. Lines starting with space are unchanged context
7. DO NOT include the entire file
8. ONLY show the lines that change plus 2-3 context lines

The step requests: "
            }
            StepCategory::Testing => {
                "Create tests for the functionality (DO NOT execute them, just create the test code). When providing test code, use XML artifact format below. Provide test code only:"
            }
            StepCategory::Documentation => {
                "\n\nCRITICAL DOCUMENTATION RULES:
                
ABSOLUTE REQUIREMENTS:
1. Create EXACTLY ONE markdown file (.md) - NO OTHER FILES
2. NEVER create separate .rs, .toml, .py, .js, .sh, or any other code files
3. NEVER create companion configuration files
4. NEVER create example files alongside documentation

FORMAT - Use ONLY this pattern:
<artifact filename=\"docs/filename.md\" type=\"markdown\">
<![CDATA[
# Documentation Title

Your documentation content here...

## Code Examples (if needed)
Include code examples using standard markdown blocks WITHOUT filenames:

```rust
fn example() {
    // code here
}
```

More documentation content...
]]>
</artifact>

WHAT YOU MUST NOT DO:
 Any code block with a filename that isn't .md

WHAT YOU MUST DO:
 Create ONE comprehensive .md file
 Put ALL content inside that single file
 Use standard markdown code blocks for examples (no filenames)"
            }
            StepCategory::Research => {
                "\n\nRESEARCH OUTPUT RULES:
1. Provide analysis in text format only
2. DO NOT create any files
3. Include findings, insights, and recommendations in your response"
            }
            StepCategory::Review => "Review the code/implementation and provide feedback:",
        };

        let format_instructions = match step.category {
            StepCategory::FileOperation
            | StepCategory::CodeGeneration
            | StepCategory::CodeModification
            | StepCategory::Testing => {
                "\n\nIMPORTANT FILE CREATION RULES:
1. YOU MUST create files using the XML artifact format below
2. Use this EXACT format for each file:
   <artifact filename=\"filename.ext\" type=\"language\">
   <![CDATA[
   entire file content here (including any markdown code blocks if this is a .md file)
   ]]>
   </artifact>

3. Examples of CORRECT format:
   <artifact filename=\"fizzbuzz.py\" type=\"python\">
   <![CDATA[
   def fizzbuzz(n):
       # implementation here
   ]]>
   </artifact>

   <artifact filename=\"README.md\" type=\"markdown\">
   <![CDATA[
   # Project Title
   
   This is a markdown file that can contain code blocks:
   
   ```python
   def example():
       return \"This code block is part of the markdown content\"
   ```
   
   ## More sections...
   ]]>
   </artifact>

4. NEVER use generic names like 'file_1.py' or 'script.py'
5. Use descriptive filenames that match the functionality
6. If implementing tests, use test_<feature>.py format
7. The CDATA section allows any content including markdown with code blocks"
            }
            StepCategory::Documentation => {
                "\n\nCRITICAL DOCUMENTATION RULES:
                
ABSOLUTE REQUIREMENTS:
1. Create EXACTLY ONE markdown file (.md) - NO OTHER FILES
2. NEVER create separate .rs, .toml, .py, .js, .sh, or any other code files
3. NEVER create companion configuration files
4. NEVER create example files alongside documentation

FORMAT - Use ONLY this pattern:
<artifact filename=\"docs/filename.md\" type=\"markdown\">
<![CDATA[
# Documentation Title

Your documentation content here...

## Code Examples (if needed)
Include code examples using standard markdown blocks WITHOUT filenames:

```rust
fn example() {
    // code here
}
```

More documentation content...
]]>
</artifact>

WHAT YOU MUST NOT DO:
 Any code block with a filename that isn't .md

WHAT YOU MUST DO:
 Create ONE comprehensive .md file
 Put ALL content inside that single file
 Use standard markdown code blocks for examples (no filenames)"
            }
            _ => "",
        };

        format!(
            "Step {}/{}: {}\n\n{}{}\n\nExecute this step precisely. Focus only on what is requested above.",
            step_num, total_steps, step.description, category_context, format_instructions
        )
    }

    fn dependencies_met(
        &self,
        _step_id: &str,
        _dependencies: &std::collections::HashMap<String, Vec<String>>,
        _completed: &[StepResult],
    ) -> bool {
        // For now, assume all dependencies are met
        // This could be enhanced to check actual dependency graph
        true
    }

    async fn extract_code_artifacts(
        &self,
        response: &str,
        _step_description: &str,
        step_category: &StepCategory,
    ) -> Result<Vec<(String, String)>> {
        let mut artifacts = Vec::new();

        // Extract code blocks with improved filename detection
        let lines: Vec<&str> = response.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            if lines[i].starts_with("<artifact") && lines[i].contains("filename=") {
                // Found an artifact block
                let mut filename = String::new();
                let mut content = String::new();
                let mut type_ = String::new();

                // Extract filename and type
                let parts: Vec<&str> = lines[i].split_whitespace().collect();
                for part in parts {
                    if part.starts_with("filename=") {
                        filename = part.trim_start_matches("filename=").trim_matches('"').to_string();
                    } else if part.starts_with("type=") {
                        type_ = part.trim_start_matches("type=").trim_matches('"').to_string();
                    }
                }

                // Collect the content
                i += 1;
                while i < lines.len() && !lines[i].starts_with("</artifact>") {
                    if lines[i].starts_with("<![CDATA[") {
                        i += 1;
                        while i < lines.len() && !lines[i].starts_with("]]>") {
                            content.push_str(lines[i]);
                            content.push('\n');
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }

                if !content.is_empty() {
                    info!("Processing artifact for step category: {:?}", step_category);
                    
                    // Check if this is placeholder/example code that should be skipped
                    let should_skip = content.lines().take(5).any(|line| {
                        let trimmed = line.trim();
                        trimmed.starts_with("# Example:")
                            || trimmed.starts_with("// Example:")
                            || trimmed.starts_with("# This is an example")
                            || trimmed.starts_with("// This is an example")
                            || (trimmed.contains("Your code goes here") && trimmed.contains("//"))
                            || (trimmed.contains("your code goes here") && trimmed.contains("#"))
                    });

                    // Check if this is generic documentation that should be skipped
                    let is_generic_doc = type_ == "markdown"
                        && (content.contains("please specify the actual")
                            || content.contains("Replace `script_name.py` with the actual")
                            || content.contains("[options]")
                            || content.contains("(if required)")
                            || content.contains("(if applicable)")
                            || (content.contains("Prerequisites")
                                && content.contains("Options & Arguments")));

                    // Check if this is a shell command that should be executed, not saved
                    let is_shell_command = (type_ == "bash"
                        || type_ == "sh"
                        || type_ == "shell")
                        && {
                            let trimmed = content.trim();
                            // Short commands (1-3 lines)
                            content.lines().count() <= 3
                                && (
                                    // Check if it starts with common command patterns
                                    trimmed.starts_with("python") ||
                            trimmed.starts_with("cargo") ||
                            trimmed.starts_with("npm") ||
                            trimmed.starts_with("yarn") ||
                            trimmed.starts_with("node") ||
                            trimmed.starts_with("git") ||
                            trimmed.starts_with("cd ") ||
                            trimmed.starts_with("mkdir") ||
                            trimmed.starts_with("./") ||
                            trimmed.starts_with("bash") ||
                            trimmed.starts_with("sh ") ||
                            // Or contains common test/run patterns
                            trimmed.contains("pytest") ||
                            trimmed.contains("unittest") ||
                            trimmed.contains("run test") ||
                            trimmed.contains("npm test") ||
                            trimmed.contains("cargo test") ||
                            // Check for pipes and redirects (common in shell commands)
                            (trimmed.contains(" | ") || trimmed.contains(" > ") || trimmed.contains(" && "))
                                )
                        };

                    if should_skip {
                        info!("Skipping example/placeholder code block");
                    } else if is_generic_doc {
                        info!("Skipping generic documentation template");
                    } else if is_shell_command {
                        info!(
                            "Skipping shell command (should be executed, not saved): {}",
                            content.lines().next().unwrap_or("")
                        );
                    } else {
                        info!(
                            "Extracted artifact: {} ({} bytes, type: {})",
                            filename,
                            content.len(),
                            type_
                        );
                        artifacts.push((filename, content.trim().to_string()));
                    }
                }
            }
            i += 1;
        }

        info!("Extracted {} artifacts from response", artifacts.len());
        Ok(artifacts)
    }
}
