use anyhow::{Result, Context as AnyhowContext};
use std::sync::Arc;
use std::collections::HashMap;

use crate::llm_manager::LLMManager;
use crate::planner::{Plan, Step, StepCategory};
use crate::artifact::{ArtifactManager, ArtifactType};
use crate::context::ContextManager;
use crate::event_bus::{EventBus, Event};
use log::info;

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
}

impl Executor {
    pub fn new() -> Self {
        Self {
            artifact_manager: None,
            context_manager: None,
            event_bus: None,
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
    pub async fn execute(&self, plan: &Plan, llm_manager: &LLMManager, context_id: &str) -> Result<Vec<StepResult>> {
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
            let result = self.execute_step(step, llm_manager, context_id, index + 1, plan.steps.len()).await
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
        llm_manager: &LLMManager,
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
        
        // Execute with the LLM
        let start_tokens = self.get_context_tokens(context_id).await;
        info!("Sending prompt to LLM for step {}", step_num);
        let response = llm_manager.send_prompt(&prompt).await
            .context("Failed to get response from LLM")?;
        info!("Received response from LLM for step {}", step_num);
        let end_tokens = self.get_context_tokens(context_id).await;
        
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
            tokens_used: end_tokens.saturating_sub(start_tokens),
            error: None,
        };
        
        // Handle category-specific post-processing
        match step.category {
            StepCategory::FileOperation | StepCategory::CodeGeneration | 
            StepCategory::CodeModification | StepCategory::Testing | 
            StepCategory::Documentation => {
                // Try to extract and save code artifacts
                if let Some(artifact_mgr) = &self.artifact_manager {
                    let artifacts = self.extract_code_artifacts(&response).await;
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
                "Modify the existing code as requested. When providing code, use markdown code blocks with the filename after the language identifier. Show the COMPLETE updated code:"
            }
            StepCategory::Testing => {
                "Create or run tests for the functionality. When providing test code, use markdown code blocks with the filename after the language identifier (e.g., ```python test_hello.py). Provide test code and expected results:"
            }
            StepCategory::Documentation => {
                "Create or update documentation. When providing documentation files, use markdown code blocks with the filename after the language identifier (e.g., ```markdown README.md). Provide the documentation content:"
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
                "\n\nIMPORTANT: When creating files, use this exact format:\n```language filename.ext\nfile content here\n```\n\nFor example:\n```python hello_world.py\ndef main():\n    print(\"Hello, World!\")\n```"
            }
            _ => ""
        };
        
        format!(
            "Step {}/{}: {}\n\n{}{}\n\nTask: {}",
            step_num,
            total_steps,
            step.description,
            category_context,
            format_instructions,
            step.description
        )
    }

    async fn get_context_tokens(&self, context_id: &str) -> usize {
        if let Some(ctx_mgr) = &self.context_manager {
            if let Ok((tokens, _)) = ctx_mgr.get_usage(context_id).await {
                return tokens;
            }
        }
        0
    }

    fn dependencies_met(&self, _step_id: &str, _dependencies: &std::collections::HashMap<String, Vec<String>>, _completed: &[StepResult]) -> bool {
        // For now, assume all dependencies are met
        // This could be enhanced to check actual dependency graph
        true
    }

    async fn extract_code_artifacts(&self, response: &str) -> Vec<(String, String)> {
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
                    // Has language and potentially filename
                    (parts[0], Some(parts[1..].join(" ")))
                } else {
                    // Only has language
                    (parts[0], None)
                };
                
                // Collect the content
                let mut content = String::new();
                i += 1;
                while i < lines.len() && !lines[i].starts_with("```") {
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                
                if !content.is_empty() {
                    // Determine filename
                    let filename = if let Some(name) = explicit_filename {
                        // Use explicitly provided filename
                        name
                    } else {
                        // Infer filename from content and context
                        code_block_counter += 1;
                        self.infer_filename(&content, language, code_block_counter)
                    };
                    
                    info!("Extracted artifact: {} ({} bytes, language: {})", filename, content.len(), language);
                    artifacts.push((filename, content.trim().to_string()));
                }
            }
            i += 1;
        }
        
        info!("Extracted {} artifacts from response", artifacts.len());
        artifacts
    }
    
    /// Infer filename from content analysis
    fn infer_filename(&self, content: &str, language: &str, counter: usize) -> String {
        // First, determine the appropriate extension
        let extension = match language {
            "python" | "py" => "py",
            "rust" | "rs" => "rs",
            "javascript" | "js" => "js",
            "typescript" | "ts" => "ts",
            "java" => "java",
            "cpp" | "c++" => "cpp",
            "c" => "c",
            "go" => "go",
            "ruby" | "rb" => "rb",
            "php" => "php",
            "shell" | "sh" | "bash" => "sh",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "markdown" | "md" => "md",
            "html" => "html",
            "css" => "css",
            _ => "txt"
        };
        
        // Try to infer filename from common patterns
        let content_lower = content.to_lowercase();
        
        // Documentation files
        if extension == "md" && (content_lower.contains("# readme") || 
                                 content_lower.starts_with("# ")) {
            return "README.md".to_string();
        }
        
        // Test files (language agnostic patterns)
        if content_lower.contains("test") && 
           (content_lower.contains("assert") || content_lower.contains("expect") ||
            content_lower.contains("describe") || content_lower.contains("it(")) {
            return format!("test_{}.{}", counter, extension);
        }
        
        // Configuration files
        if extension == "txt" && content_lower.contains("==") && 
           (content_lower.contains("pip") || content_lower.contains("requirements")) {
            return "requirements.txt".to_string();
        }
        
        if (extension == "json" || extension == "yaml" || extension == "toml") &&
           (content_lower.contains("dependencies") || content_lower.contains("version")) {
            return format!("config.{}", extension);
        }
        
        // Gitignore
        if content.lines().any(|line| line.starts_with("*.") || line.starts_with("/")) &&
           content.lines().count() > 2 {
            return ".gitignore".to_string();
        }
        
        // Main/entry point files (language agnostic)
        if content_lower.contains("main") || content_lower.contains("entry") ||
           content_lower.contains("start") || content_lower.contains("init") {
            return format!("main.{}", extension);
        }
        
        // Generic fallback with more descriptive name
        format!("file_{}.{}", counter, extension)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
