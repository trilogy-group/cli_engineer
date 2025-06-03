use anyhow::{Result, Context as AnyhowContext};
use std::sync::Arc;
use std::collections::HashMap;

use crate::llm_manager::LLMManager;
use crate::planner::{Plan, Step, StepCategory};
use crate::artifact::{ArtifactManager, ArtifactType};
use crate::context::ContextManager;
use crate::event_bus::{EventBus, Event};
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
}

impl Executor {
    pub fn new(llm_manager: Arc<LLMManager>) -> Self {
        Self {
            artifact_manager: None,
            context_manager: None,
            event_bus: None,
            llm_manager,
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

    /// Execute the entire plan and return results for each step
    pub async fn execute(&self, plan: &Plan, context_id: &str) -> Result<Vec<StepResult>> {
        let mut results = Vec::new();
        
        // Emit plan execution started event
        if let Some(bus) = &self.event_bus {
            let _ = bus.emit(Event::Custom {
                event_type: "plan_execution_started".to_string(),
                data: serde_json::json!({
                    "plan_goal": plan.goal,
                    "total_steps": plan.steps.len(),
                    "complexity": format!("{:?}", plan.estimated_complexity)
                }),
            }).await;
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
            let result = self.execute_step(step, context_id, index + 1, plan.steps.len()).await
                .context(format!("Failed to execute step: {}", step.description))?;
            
            // Emit step completed event
            if let Some(bus) = &self.event_bus {
                let _ = bus.emit(Event::TaskProgress {
                    task_id: step.id.clone(),
                    progress: ((index + 1) as f32 / plan.steps.len() as f32) * 100.0,
                    message: format!("Completed step {}/{}: {}", index + 1, plan.steps.len(), step.description),
                }).await;
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
        info!("Executing step {}/{}: {}", step_num, total_steps, step.description);
        
        // Build the appropriate prompt based on step category
        let prompt = self.build_step_prompt(step, step_num, total_steps);
        
        // Add to context if available
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr.add_message(
                context_id,
                "user".to_string(),
                format!("Step {}: {}", step_num, step.description),
            ).await?;
        }
        
        // Send to LLM
        let response = self.llm_manager.send_prompt(&prompt).await?;
        
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
            ctx_mgr.add_message(context_id, "assistant".to_string(), response.clone()).await?;
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
            StepCategory::FileOperation | StepCategory::CodeGeneration | 
            StepCategory::CodeModification | StepCategory::Testing | 
            StepCategory::Documentation => {
                // Try to extract and save code artifacts
                if let Some(artifact_mgr) = &self.artifact_manager {
                    let artifacts = self.extract_code_artifacts(&response, &step.description, &step.category).await?;
                    for (filename, content) in artifacts {
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
                        
                        match artifact_mgr.create_artifact(
                            filename.clone(),
                            artifact_type,
                            content.clone(),
                            metadata,
                        ).await {
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
                "Analyze the following requirement and provide detailed insights:"
            }
            StepCategory::FileOperation => {
                "Create or modify the specified file. When providing code, use markdown code blocks with the filename after the language identifier (e.g., ```python hello.py). Provide the COMPLETE file content:"
            }
            StepCategory::CodeGeneration => {
                "Generate the requested code. When providing code, use markdown code blocks with the filename after the language identifier (e.g., ```python hello.py). Provide COMPLETE, working code:"
            }
            StepCategory::CodeModification => {
                "Modify the existing code as requested. 

YOU MUST use diff format. Here's EXACTLY what to output:

```diff
--- filename.ext
+++ filename.ext
@@ -5,3 +5,4 @@
 def hello():
-    print('Hello')
+    print('Hello, World!')
+    return True
```

RULES:
1. ALWAYS start with ```diff (NO filename after diff)
2. Use --- filename.ext and +++ filename.ext headers
3. Use @@ -old_line,old_count +new_line,new_count @@
4. Lines starting with - are removed
5. Lines starting with + are added
6. Lines starting with space are unchanged context
7. DO NOT include the entire file
8. ONLY show the lines that change plus 2-3 context lines

The step requests: "
            }
            StepCategory::Testing => {
                "Create tests for the functionality (DO NOT execute them, just create the test code). When providing test code, use markdown code blocks with the filename after the language identifier (e.g., ```python test_hello.py). Provide test code only:"
            }
            StepCategory::Documentation => {
                "Create or update documentation. CRITICAL: If adding comments/docstrings to existing code, use 'Code Modification' format with diff blocks instead of creating new files. Only create separate documentation files (like README.md) if explicitly requested. Focus on the specific task at hand - do not create unrelated documentation or example files:"
            }
            StepCategory::Research => {
                "Research the following topic and provide findings:"
            }
            StepCategory::Review => {
                "Review the code/implementation and provide feedback:"
            }
        };
        
        let format_instructions = match step.category {
            StepCategory::FileOperation | StepCategory::CodeGeneration | 
            StepCategory::CodeModification | StepCategory::Testing | 
            StepCategory::Documentation => {
                "\n\nIMPORTANT FILE CREATION RULES:
1. YOU MUST ALWAYS include the filename after the language in code blocks
2. Use this EXACT format:
   ```language filename.ext
   file content here
   ```

3. Examples of CORRECT format:
   ```python fizzbuzz.py
     def fizzbuzz(n):
         # implementation
   ```

   ```python requirements.txt
     flask==2.0.1
     requests==2.28.0
   ```

   ```markdown README.md
     # Project Title
     Description here
   ```

4. NEVER use generic names like 'file_1.py' or 'script.py'
5. Use descriptive filenames that match the functionality
6. If implementing tests, use test_<feature>.py format

7. DO NOT include:
   - Shell commands in code blocks (explain how to run in text)
   - Reasoning or explanations in code blocks
   - Files from unrelated tasks
   - Example or placeholder code (unless specifically requested)
   - Test execution commands (like 'pytest test.py' or 'python script.py')

8. CRITICAL: Only generate code directly related to THIS SPECIFIC task:
   - DO NOT add unrelated test files or examples
   - DO NOT create files for tasks not mentioned in the step description
   - If asked for FizzBuzz, DO NOT create factorial or other unrelated code
   - Stay focused ONLY on what is explicitly requested
   - DO NOT create utility functions unless they are specifically needed for this task
   - DO NOT add bonus features or demonstrate other algorithms

9. CONTEXT AWARENESS:
   - If the step mentions specific files, ONLY work with those files
   - If the step says 'add comment to file X', modify file X, don't create new files
   - If no specific file is mentioned, ask yourself: what file would logically need this change?
   - When in doubt, create minimal, focused code that directly addresses the step description

FAILURE TO INCLUDE PROPER FILENAMES WILL RESULT IN GENERIC NAMES LIKE 'file_1.py'"
            }
            _ => ""
        };
        
        format!(
            "Step {}/{}: {}\n\n{}{}\n\nExecute this step precisely. Focus only on what is requested above.",
            step_num,
            total_steps,
            step.description,
            category_context,
            format_instructions
        )
    }

    fn dependencies_met(&self, _step_id: &str, _dependencies: &std::collections::HashMap<String, Vec<String>>, _completed: &[StepResult]) -> bool {
        // For now, assume all dependencies are met
        // This could be enhanced to check actual dependency graph
        true
    }

    async fn apply_diff_patch(&self, filename: &str, diff_content: &str) -> Result<()> {
        if let Some(artifact_mgr) = &self.artifact_manager {
            // Try to find the existing artifact
            let artifacts = artifact_mgr.list_artifacts().await;
            let existing_artifact = artifacts.iter()
                .find(|a| a.name == filename || a.path.file_name().map(|f| f.to_string_lossy()) == Some(filename.into()));
            
            let existing_content = if let Some(artifact) = existing_artifact {
                // Read the file content from disk if not in memory
                if let Some(content) = &artifact.content {
                    content.clone()
                } else {
                    std::fs::read_to_string(&artifact.path)
                        .unwrap_or_else(|_| {
                            warn!("Could not read file {}", artifact.path.display());
                            String::new()
                        })
                }
            } else {
                warn!("File {} doesn't exist for modification, creating new file", filename);
                String::new()
            };
            
            // Apply the diff to get the new content
            let new_content = Self::apply_unified_diff(&existing_content, diff_content)?;
            
            // Determine the artifact type based on extension
            let extension = filename.split('.').last();
            let artifact_type = match extension {
                Some("rs") => ArtifactType::SourceCode,
                Some("py") => ArtifactType::SourceCode,
                Some("js") | Some("ts") => ArtifactType::SourceCode,
                Some("toml") | Some("yaml") | Some("yml") => ArtifactType::Configuration,
                Some("md") => ArtifactType::Documentation,
                _ => ArtifactType::Other("modified".to_string()),
            };
            
            // Create metadata
            let mut metadata = HashMap::new();
            metadata.insert("description".to_string(), format!("Modified via diff patch"));
            metadata.insert("modified_by".to_string(), "diff_patch".to_string());
            
            // Update or create the artifact
            if existing_artifact.is_some() {
                // Update existing artifact
                artifact_mgr.update_artifact(
                    &existing_artifact.unwrap().id,
                    new_content,
                ).await?;
                info!("Updated {} via diff patch", filename);
            } else {
                // Create new artifact
                artifact_mgr.create_artifact(
                    filename.to_string(),
                    artifact_type,
                    new_content,
                    metadata,
                ).await?;
                info!("Created {} from diff patch", filename);
            }
        }
        
        Ok(())
    }

    /// Parse and apply a unified diff to content
    fn apply_unified_diff(original: &str, diff: &str) -> Result<String> {
        let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();
        
        // Simple diff parser - handles basic unified diff format
        let diff_lines: Vec<&str> = diff.lines().collect();
        let mut i = 0;
        
        while i < diff_lines.len() {
            let line = diff_lines[i];
            
            // Look for hunk header: @@ -start,count +start,count @@
            if line.starts_with("@@") && line.contains("@@") {
                // Parse the hunk header
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    // Extract line numbers
                    let old_range = parts[1].trim_start_matches('-');
                    
                    let old_start: usize = old_range.split(',')
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(1)
                        .saturating_sub(1); // Convert to 0-based
                    
                    // Process the hunk
                    i += 1;
                    let mut current_line = old_start;
                    let mut new_lines = Vec::new();
                    
                    while i < diff_lines.len() && !diff_lines[i].starts_with("@@") {
                        let diff_line = diff_lines[i];
                        
                        if diff_line.starts_with('-') {
                            // Remove line
                            current_line += 1;
                        } else if diff_line.starts_with('+') {
                            // Add line
                            new_lines.push(diff_line[1..].to_string());
                        } else if diff_line.starts_with(' ') {
                            // Context line
                            if current_line < lines.len() {
                                new_lines.push(lines[current_line].clone());
                            }
                            current_line += 1;
                        }
                        
                        i += 1;
                    }
                    
                    // Apply the changes
                    // This is a simplified approach - a real implementation would be more sophisticated
                    if old_start < lines.len() {
                        lines.splice(old_start..current_line.min(lines.len()), new_lines);
                    }
                }
            } else {
                i += 1;
            }
        }
        
        Ok(lines.join("\n"))
    }

    async fn extract_code_artifacts(&self, response: &str, step_description: &str, step_category: &StepCategory) -> Result<Vec<(String, String)>> {
        let mut artifacts = Vec::new();
        
        // Extract code blocks with improved filename detection
        let lines: Vec<&str> = response.lines().collect();
        let mut i = 0;
        let mut code_block_counter = 0;
        
        while i < lines.len() {
            if lines[i].starts_with("```") && lines[i].len() > 3 {
                // Found a code block
                let header = lines[i].trim_start_matches("```").trim();
                let parts: Vec<&str> = header.split_whitespace().collect();
                
                // Try to extract language and filename
                let (language, explicit_filename) = if parts.is_empty() {
                    ("txt", None)
                } else if parts.len() >= 2 {
                    // Format: ```language filename
                    if parts[0] == "diff" {
                        // Special handling for diff blocks: ```diff filename.ext
                        ("diff", Some(parts[1..].join(" ")))
                    } else {
                        // Regular code blocks: ```language filename.ext
                        (parts[0], Some(parts[1..].join(" ")))
                    }
                } else {
                    // Format: ```language
                    (parts[0], None)
                };
                
                // Check if this is a diff block
                let is_diff = language == "diff";
                
                // Collect the content
                let mut content = String::new();
                i += 1;
                while i < lines.len() && !lines[i].starts_with("```") {
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                
                if !content.is_empty() {
                    // Check if this is placeholder/example code that should be skipped
                    let should_skip = content.lines().take(5).any(|line| {
                        let trimmed = line.trim();
                        trimmed.starts_with("# Example:") ||
                        trimmed.starts_with("// Example:") ||
                        trimmed.starts_with("# This is an example") ||
                        trimmed.starts_with("// This is an example") ||
                        (trimmed.contains("Your code goes here") && trimmed.contains("//")) ||
                        (trimmed.contains("your code goes here") && trimmed.contains("#"))
                    });
                    
                    // Check if this is generic documentation that should be skipped
                    let is_generic_doc = language == "markdown" && (
                        content.contains("please specify the actual") ||
                        content.contains("Replace `script_name.py` with the actual") ||
                        content.contains("[options]") ||
                        content.contains("(if required)") ||
                        content.contains("(if applicable)") ||
                        (content.contains("Prerequisites") && content.contains("Options & Arguments"))
                    );
                    
                    // Check if this is a shell command that should be executed, not saved
                    let is_shell_command = (language == "bash" || language == "sh" || language == "shell") && {
                        let trimmed = content.trim();
                        // Short commands (1-3 lines)
                        content.lines().count() <= 3 && (
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
                        info!("Skipping shell command (should be executed, not saved): {}", content.lines().next().unwrap_or(""));
                    } else if is_diff {
                        // Apply the diff patch
                        // First try to extract filename from diff headers
                        let diff_filename = Self::extract_filename_from_diff(&content).or(explicit_filename.as_ref().map(|s| s.as_str()));
                        
                        if let Some(name) = diff_filename {
                            self.apply_diff_patch(name, &content).await?;
                        } else {
                            warn!("Diff block without filename, skipping");
                        }
                    } else {
                        // For CodeModification steps, only create new files if they don't exist
                        // Otherwise, the LLM should be providing diff blocks to modify existing files
                        if let StepCategory::CodeModification = step_category {
                            if let Some(ref name) = explicit_filename {
                                // Check if file already exists
                                if let Some(artifact_mgr) = &self.artifact_manager {
                                    if artifact_mgr.artifact_exists(name).await {
                                        warn!("CodeModification step provided non-diff code block for existing file '{}'. Skipping to avoid overwrite. LLM should provide diff format for modifications.", name);
                                        continue;
                                    }
                                }
                            }
                        }
                        
                        // Determine filename
                        let filename = if let Some(name) = explicit_filename {
                            // Use explicitly provided filename
                            name
                        } else {
                            // Ask LLM for a proper filename
                            code_block_counter += 1;
                            
                            // Create a prompt to ask for filename
                            let context_lines = if language == "markdown" { 20 } else { 10 };
                            let filename_prompt = format!(
                                "Based on the following {} code from the task '{}', please provide a descriptive filename:\n\n```{}\n{}\n```\n\nProvide ONLY the filename with extension, nothing else. For markdown documentation, use README.md if it's the main documentation.",
                                language,
                                step_description,
                                language,
                                content.lines().take(context_lines).collect::<Vec<_>>().join("\n")
                            );
                            
                            // Try to get filename from LLM
                            match self.llm_manager.send_prompt(&filename_prompt).await {
                                Ok(filename_response) => {
                                    let suggested_name = filename_response.trim();
                                    // Validate the suggested filename
                                    if suggested_name.contains('.') && 
                                       !suggested_name.contains(' ') && 
                                       !suggested_name.contains('/') &&
                                       suggested_name.len() < 100 {
                                        suggested_name.to_string()
                                    } else {
                                        // If invalid, use fallback
                                        warn!("LLM provided invalid filename: {}", suggested_name);
                                        format!("file_{}.{}", code_block_counter, language)
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to get filename from LLM: {}", e);
                                    // Fallback to generic name
                                    format!("file_{}.{}", code_block_counter, language)
                                }
                            }
                        };
                        
                        // Skip files marked as examples by the inference logic
                        if !filename.starts_with("example_") {
                            info!("Extracted artifact: {} ({} bytes, language: {})", filename, content.len(), language);
                            artifacts.push((filename, content.trim().to_string()));
                        } else {
                            info!("Skipping inferred example file: {}", filename);
                        }
                    }
                }
            }
            i += 1;
        }
        
        info!("Extracted {} artifacts from response", artifacts.len());
        Ok(artifacts)
    }
    
    fn extract_filename_from_diff(diff: &str) -> Option<&str> {
        let lines: Vec<&str> = diff.lines().collect();
        for line in lines {
            if line.starts_with("--- ") || line.starts_with("+++ ") {
                let filename = line.trim_start_matches("--- ").trim_start_matches("+++ ");
                if filename.contains('.') {
                    return Some(filename);
                }
            }
        }
        None
    }
}
