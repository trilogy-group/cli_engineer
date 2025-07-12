use anyhow::{Context as AnyhowContext, Result};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    artifact::{ArtifactManager, ArtifactType},
    command_executor::CommandExecutor,
    context::ContextManager,
    event_bus::{Event, EventBus},
    llm_manager::LLMManager,
    planner::{Plan, Step, StepCategory},
    tool_manager::ToolManager,
    CommandKind,
};
use log::{info, warn};

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
    command_executor: Option<Arc<CommandExecutor>>,
    tool_manager: Option<Arc<ToolManager>>,
}

impl Executor {
    pub fn new(llm_manager: Arc<LLMManager>) -> Self {
        Self {
            artifact_manager: None,
            context_manager: None,
            event_bus: None,
            llm_manager,
            command: None,
            command_executor: None,
            tool_manager: None,
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

    pub fn with_command_executor(mut self, command_executor: Arc<CommandExecutor>) -> Self {
        self.command_executor = Some(command_executor);
        self
    }

    pub fn with_tool_manager(mut self, tool_manager: Arc<ToolManager>) -> Self {
        self.tool_manager = Some(tool_manager);
        self
    }

    /// Execute the plan with iteration context for hybrid progress tracking
    pub async fn execute_with_progress(
        &self, 
        plan: &Plan, 
        context_id: &str, 
        current_iteration: usize,
        max_iterations: usize
    ) -> Result<Vec<StepResult>> {
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
            // Emit detailed step progress for hybrid model
            if let Some(bus) = &self.event_bus {
                let base_progress = (current_iteration - 1) as f32 / max_iterations as f32;
                let step_progress = (index as f32) / (plan.steps.len() as f32);
                let execution_phase_weight = 0.7 / max_iterations as f32; // Execution is 70% of iteration
                let total_progress = base_progress + 0.1 / max_iterations as f32 + (step_progress * execution_phase_weight);
                
                let step_type = match step.category {
                    StepCategory::Analysis => "Analysis",
                    StepCategory::CodeGeneration => "Code Generation",
                    StepCategory::CodeModification => "Code Modification",
                    StepCategory::FileOperation => "File Operation",
                    StepCategory::CommandExecution => "Command Execution",
                    StepCategory::Testing => "Testing",
                    StepCategory::Documentation => "Documentation",
                    StepCategory::Review => "Review",
                    StepCategory::Research => "Research",
                    StepCategory::ToolInvocation => "Tool Invocation",
                };
                let phase = format!("Iteration {}/{}: Step {}/{} - {}", 
                    current_iteration, max_iterations, index + 1, plan.steps.len(), step_type);
                
                let _ = bus.emit(Event::ExecutionProgress {
                    step: phase,
                    progress: total_progress,
                }).await;
            }

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
                .execute_step(step, context_id, index + 1, plan.steps.len(), &results)
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

        // Emit completion event
        if let Some(bus) = &self.event_bus {
            let _ = bus.emit(Event::ExecutionProgress {
                step: "Plan execution completed".to_string(),
                progress: 1.0,
            }).await;
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
        previous_results: &[StepResult],
    ) -> Result<StepResult> {
        info!(
            "Executing step {}/{}: {}",
            step_num, total_steps, step.description
        );

        // Build the appropriate prompt based on step category
        let base_prompt = self.build_step_prompt(step, step_num, total_steps, previous_results);

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
            StepCategory::CommandExecution => {
                // Execute shell commands based on LLM response
                if let Some(cmd_executor) = &self.command_executor {
                    // Parse commands from the LLM response
                    let commands = self.extract_commands_from_response(&response);
                    let mut command_outputs = Vec::new();
                    
                    info!("Command Execution Step: Found {} commands to execute", commands.len());
                    for (i, command) in commands.iter().enumerate() {
                        info!("  Command {}: {}", i + 1, command);
                    }
                    
                    for command in commands {
                        match cmd_executor.execute_command(&command, None).await {
                            Ok(cmd_result) => {
                                info!("{}", cmd_result.summary());
                                command_outputs.push(cmd_result.get_output());
                                
                                // If any command fails, mark the step as failed
                                if !cmd_result.is_success() {
                                    result.success = false;
                                    result.error = Some(format!("Command failed: {}", cmd_result.summary()));
                                }
                            }
                            Err(e) => {
                                warn!("Command '{}' failed: {}", command, e);
                                result.success = false;
                                result.error = Some(format!("Command execution failed: {}", e));
                                command_outputs.push(format!("Command: {}\nStatus: FAILED\nError: {}", command, e));
                            }
                        }
                    }
                    
                    // Update the result with command outputs
                    result.output = command_outputs.join("\n\n");
                }
            }
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
            StepCategory::ToolInvocation => {
                // Handle MCP tool invocation
                if let Some(tool_mgr) = &self.tool_manager {
                    // Extract tool name and arguments from the response
                    let (tool_name, tool_args) = self.extract_tool_call_from_response(&response)?;
                    
                    info!("Invoking MCP tool '{}' with args: {}", tool_name, tool_args);
                    
                    match tool_mgr.invoke_tool(&tool_name, &tool_args).await {
                        Ok(tool_result) => {
                            info!("Tool '{}' invocation successful", tool_name);
                            result.output = tool_result;
                        }
                        Err(e) => {
                            warn!("Tool '{}' invocation failed: {}", tool_name, e);
                            result.success = false;
                            result.error = Some(format!("Tool invocation failed: {}", e));
                        }
                    }
                } else {
                    warn!("Tool invocation requested but no ToolManager available");
                    result.success = false;
                    result.error = Some("Tool invocation not available - ToolManager not configured".to_string());
                }
            }
            _ => {
                // Other categories don't typically create artifacts
            }
        }

        Ok(result)
    }

    fn build_step_prompt(&self, step: &Step, step_num: usize, total_steps: usize, previous_results: &[StepResult]) -> String {
        // Build context from previous step results, especially command outputs
        let mut previous_context = String::new();
        if !previous_results.is_empty() {
            previous_context.push_str("\n\nPREVIOUS STEP RESULTS:\n");
            
            // Include recent command execution results (last 3 steps)
            let recent_results = previous_results.iter().rev().take(3).collect::<Vec<_>>();
            for (i, result) in recent_results.iter().rev().enumerate() {
                let step_index = previous_results.len() - recent_results.len() + i + 1;
                previous_context.push_str(&format!("\n--- Step {} ({}) ---\n", 
                    step_index, 
                    if result.success { "SUCCESS" } else { "FAILED" }
                ));
                
                // Include command outputs, especially for failed commands or important results
                if !result.output.is_empty() {
                    let output = if result.output.len() > 800 {
                        // Truncate very long outputs but keep error messages
                        let mut truncate_at = 800;
                        while truncate_at > 0 && !result.output.is_char_boundary(truncate_at) {
                            truncate_at -= 1;
                        }
                        format!("{}... [truncated]", &result.output[..truncate_at])
                    } else {
                        result.output.clone()
                    };
                    previous_context.push_str(&format!("Output:\n{}\n", output));
                }
                
                if let Some(error) = &result.error {
                    previous_context.push_str(&format!("Error: {}\n", error));
                }
                
                if !result.artifacts_created.is_empty() {
                    previous_context.push_str(&format!("Artifacts created: {}\n", 
                        result.artifacts_created.join(", ")));
                }
            }
            
            previous_context.push_str("\nUse the above results to inform your current step. If there were compilation errors, test failures, or other issues, address them in this step.\n");
        }
        
        let category_context: String = match step.category {
            StepCategory::Analysis => {
                "\n\nANALYSIS RULES:
1. Provide analysis in text format only
2. DO NOT create any files
3. Include findings, code analysis, and recommendations in your response:".to_string()
            }
            StepCategory::FileOperation => {
                "Create or modify the specified file. When providing code, use XML artifact format below. Provide the COMPLETE file content:".to_string()
            }
            StepCategory::CodeGeneration => {
                "Generate the requested code. When providing code, use XML artifact format below. Provide COMPLETE and FUNCTIONAL code:".to_string()
            }
            StepCategory::CodeModification => {
                "Modify the existing code as requested. When providing code, use XML artifact format below. Provide the COMPLETE modified file:".to_string()
            }
            StepCategory::Testing => {
                "Create tests for the functionality (DO NOT execute them, just create the test code). When providing test code, use XML artifact format below. Provide test code only:".to_string()
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
 Use standard markdown code blocks for examples (no filenames)".to_string()
            }
            StepCategory::Research => {
                "\n\nRESEARCH OUTPUT RULES:
1. Provide analysis in text format only
2. DO NOT create any files
3. Include findings, insights, and recommendations in your response".to_string()
            }
            StepCategory::Review => "Review the code/implementation and provide feedback:".to_string(),
            StepCategory::CommandExecution => {
                if let Some(cmd_executor) = &self.command_executor {
                    let commands = cmd_executor.get_allowed_commands();
                    let formatted_commands = commands.iter().map(|c| format!("   - {}", c)).collect::<Vec<_>>().join("\n");
                    info!("Command Execution: Passing {} allowlisted commands to LLM", commands.len());
                    info!("Allowlisted commands: {}", formatted_commands);
                    format!(
                        "\n\n🚨 CRITICAL: COMMAND EXECUTION STEP 🚨\n\nYOU MUST FOLLOW THIS EXACT FORMAT:\n\n1. If commands ARE needed, provide them in a bash code block:\n```bash\ncommand1\ncommand2\n```\n\n2. If NO commands are needed, respond with EXACTLY:\n```\nNO_COMMANDS_NEEDED\n```\n\nSTRICT RULES:\n✅ ONLY provide shell commands in bash blocks\n✅ Commands must start with one of these allowed prefixes:\n{}\n✅ One command per line\n✅ NO explanatory text outside code blocks\n✅ NO simulated outputs or fake logs\n✅ NO analysis or descriptions\n\nEXAMPLES OF CORRECT RESPONSES:\n\nExample 1 (commands needed):\n```bash\ncargo build\ncargo test\n```\n\nExample 2 (no commands needed):\n```\nNO_COMMANDS_NEEDED\n```\n\nThe system will EXECUTE these commands and return REAL output!",
                        formatted_commands
                    )
                } else {
                    "\n\nCOMMAND EXECUTION: Command execution is not available.\n".to_string()
                }
            }
            StepCategory::ToolInvocation => {
                if let Some(tool_mgr) = &self.tool_manager {
                    let tools = tool_mgr.list_tools();
                    let tool_list = tools.iter()
                        .map(|t| format!("   - {}: {}", t.name, t.description.as_deref().unwrap_or("No description")))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "\n\n🛠️ TOOL INVOCATION STEP 🛠️\n\nYOU MUST FOLLOW THIS EXACT FORMAT:\n\n1. Specify the tool to invoke:\nTool: tool_name\nArguments: {{\"key\": \"value\"}}\n\n2. Or use this format:\nCall tool_name with {{\"key\": \"value\"}}\n\n3. Or use JSON format:\n```json\n{{\"tool\": \"tool_name\", \"args\": {{\"key\": \"value\"}}}}\n```\n\nAVAILABLE TOOLS:\n{}\n\nEXAMPLE:\nTool: search_files\nArguments: {{\"query\": \"config file\", \"path\": \"/src\"}}\n\nThe system will invoke the tool and return the result!",
                        tool_list
                    )
                } else {
                    "\n\nTOOL INVOCATION: No tools are available.\n".to_string()
                }
            }
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
            StepCategory::CommandExecution => {
                "\n\nCOMMAND OUTPUT FORMAT:\n1. Provide commands in bash code blocks or with $ prefix\n2. Explain what each command does and why it's needed\n3. The system will execute these commands and show you the results\n4. You can then analyze the results and provide insights"
            }
            _ => "",
        };

        format!(
            "Step {}/{}: {}{}\n\n{}{}\n\nExecute this step precisely. Focus only on what is requested above.",
            step_num, total_steps, step.description, previous_context, category_context, format_instructions
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

    /// Extract shell commands from LLM response
    fn extract_commands_from_response(&self, response: &str) -> Vec<String> {
        let mut commands = Vec::new();
        let lines = response.lines();
        
        let mut in_code_block = false;
        let mut code_block_type = String::new();
        let mut code_block_content: Vec<String> = Vec::new();
        
        // First check if the response indicates no commands are needed
        if response.contains("NO_COMMANDS_NEEDED") {
            info!("LLM explicitly indicated no commands are needed");
            return commands; // Return empty vec, but this is intentional
        }
        
        for line in lines {
            let trimmed = line.trim();
            
            // Detect code block start
            if trimmed.starts_with("```") {
                if in_code_block {
                    // End of code block - process accumulated content
                    if code_block_type == "bash" || code_block_type == "shell" || code_block_type == "sh" || code_block_type.is_empty() {
                        // Process the code block content
                        for cmd_line in &code_block_content {
                            let cmd_trimmed = cmd_line.trim();
                            if !cmd_trimmed.is_empty() && !cmd_trimmed.starts_with('#') {
                                commands.push(cmd_trimmed.to_string());
                            }
                        }
                    }
                    code_block_content.clear();
                    in_code_block = false;
                    code_block_type.clear();
                } else {
                    // Start of code block
                    in_code_block = true;
                    code_block_type = trimmed[3..].trim().to_lowercase();
                }
                continue;
            }
            
            // Collect code block content
            if in_code_block {
                code_block_content.push(line.to_string());
                continue;
            }
            
            // Fallback patterns for commands outside code blocks (less preferred)
            if !in_code_block {
                // Pattern 1: Commands prefixed with $
                if trimmed.starts_with("$ ") {
                    let command = trimmed[2..].trim();
                    if !command.is_empty() {
                        commands.push(command.to_string());
                    }
                }
                // Pattern 2: Backticked commands
                else if trimmed.starts_with("`") && trimmed.ends_with("`") && trimmed.len() > 2 {
                    let potential_command = &trimmed[1..trimmed.len()-1];
                    // Validate it looks like a real command
                    if self.looks_like_command(potential_command) {
                        commands.push(potential_command.to_string());
                    }
                }
            }
        }
        
        info!("Extracted {} commands from response", commands.len());
        if commands.is_empty() && !response.contains("NO_COMMANDS_NEEDED") {
            warn!("No commands extracted from Command Execution step response - this may indicate a malformed response");
        }
        commands
    }
    
    /// Check if a string looks like a valid command based on allowed prefixes
    fn looks_like_command(&self, potential_command: &str) -> bool {
        if let Some(cmd_executor) = &self.command_executor {
            let allowed_commands = cmd_executor.get_allowed_commands();
            // Check if the command starts with any allowed prefix
            allowed_commands.iter().any(|allowed| {
                let prefix = allowed.split_whitespace().next().unwrap_or("");
                potential_command.starts_with(prefix)
            })
        } else {
            false
        }
    }
    
    /// Extract tool name and arguments from LLM response for tool invocation
    fn extract_tool_call_from_response(&self, response: &str) -> Result<(String, String)> {
        // Look for patterns like:
        // Tool: tool_name
        // Arguments: {"key": "value"}
        // Or:
        // Call tool_name with {"key": "value"}
        // Or in JSON blocks:
        // ```json
        // {"tool": "tool_name", "args": {"key": "value"}}
        // ```
        
        let lines: Vec<&str> = response.lines().collect();
        let mut tool_name = String::new();
        let mut tool_args = String::new();
        let mut in_json_block = false;
        let mut json_content = String::new();
        
        for line in &lines {
            let trimmed = line.trim();
            
            // Check for JSON code blocks
            if trimmed == "```json" {
                in_json_block = true;
                json_content.clear();
                continue;
            }
            if in_json_block && trimmed == "```" {
                // Try to parse the JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_content) {
                    if let Some(tool) = json.get("tool").and_then(|v| v.as_str()) {
                        tool_name = tool.to_string();
                    }
                    if let Some(args) = json.get("args") {
                        tool_args = serde_json::to_string(args).unwrap_or_default();
                    }
                }
                in_json_block = false;
                continue;
            }
            if in_json_block {
                json_content.push_str(trimmed);
                json_content.push('\n');
                continue;
            }
            
            // Pattern: "Tool: tool_name"
            if trimmed.starts_with("Tool:") || trimmed.starts_with("tool:") {
                tool_name = trimmed[5..].trim().to_string();
            }
            // Pattern: "Arguments: {...}"
            else if (trimmed.starts_with("Arguments:") || trimmed.starts_with("args:")) && !tool_name.is_empty() {
                tool_args = trimmed[10..].trim().to_string();
            }
            // Pattern: "Call tool_name with {...}"
            else if trimmed.starts_with("Call ") && trimmed.contains(" with ") {
                let parts: Vec<&str> = trimmed[5..].split(" with ").collect();
                if parts.len() == 2 {
                    tool_name = parts[0].trim().to_string();
                    tool_args = parts[1].trim().to_string();
                }
            }
            // Pattern: "Use 'tool_name' with {...}"
            else if trimmed.starts_with("Use '") && trimmed.contains("' with ") {
                let start = 5;
                if let Some(end) = trimmed[start..].find("'") {
                    tool_name = trimmed[start..start+end].to_string();
                    if let Some(with_pos) = trimmed.find("' with ") {
                        tool_args = trimmed[with_pos + 7..].trim().to_string();
                    }
                }
            }
        }
        
        if tool_name.is_empty() {
            anyhow::bail!("Could not extract tool name from response");
        }
        if tool_args.is_empty() {
            tool_args = "{}".to_string();
        }
        
        Ok((tool_name, tool_args))
    }
}
