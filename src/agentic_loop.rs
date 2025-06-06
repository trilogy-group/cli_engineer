use crate::{
    artifact::{ArtifactManager, ArtifactType},
    config::Config,
    context::ContextManager,
    event_bus::{Event, EventBus},
    executor::{Executor, StepResult},
    interpreter::Interpreter,
    iteration_context::{FileInfo, IterationContext},
    llm_manager::LLMManager,
    planner::{Plan, Planner},
    reviewer::{IssueSeverity, ReviewResult, Reviewer},
    CommandKind,
};
use anyhow::Result;
use log::{error, info, warn};
use std::sync::Arc;

/// Controls the iterative planning-action-review cycle
pub struct AgenticLoop {
    interpreter: Interpreter,
    planner: Planner,
    executor: Executor,
    reviewer: Reviewer,
    llm_manager: Arc<LLMManager>,
    max_iterations: usize,
    event_bus: Arc<EventBus>,
    artifact_manager: Option<Arc<ArtifactManager>>,
    context_manager: Option<Arc<ContextManager>>,
    config: Option<Arc<Config>>,
    command: Option<CommandKind>,
}

impl AgenticLoop {
    pub fn new(
        llm_manager: Arc<LLMManager>,
        max_iterations: usize,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            interpreter: Interpreter::new(),
            planner: Planner::new(),
            executor: Executor::new(llm_manager.clone()).with_event_bus(event_bus.clone()),
            reviewer: Reviewer::new().with_event_bus(event_bus.clone()),
            llm_manager,
            max_iterations,
            event_bus,
            artifact_manager: None,
            context_manager: None,
            config: None,
            command: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_artifact_manager(mut self, manager: Arc<ArtifactManager>) -> Self {
        self.executor = self.executor.with_artifact_manager(manager.clone());
        self.artifact_manager = Some(manager);
        self
    }

    #[allow(dead_code)]
    pub fn with_context_manager(mut self, manager: Arc<ContextManager>) -> Self {
        self.context_manager = Some(manager.clone());
        
        // Also set the context manager on the executor and reviewer
        self.executor = self.executor.with_context_manager(manager.clone());
        self.reviewer = self.reviewer.with_context_manager(manager);
        
        self
    }

    pub fn with_config(mut self, config: Arc<Config>) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_command(mut self, command: CommandKind) -> Self {
        self.executor = self.executor.with_command(command.clone());
        self.command = Some(command);
        self
    }

    /// Run the agentic loop on the given input
    pub async fn run(&self, input: &str, context_id: &str) -> Result<()> {
        info!("Starting agentic loop for input: {}", input);

        // Interpret the task
        let task = self.interpreter.interpret(input)?;
        info!("Interpreted task: {}", task.description);

        // Add initial task to context
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr
                .add_message(context_id, "user".to_string(), input.to_string())
                .await?;
            ctx_mgr
                .add_message(
                    context_id,
                    "system".to_string(),
                    format!(
                        "Task interpreted as: {}\nGoal: {}",
                        task.description, task.goal
                    ),
                )
                .await?;
        }

        let mut iteration = 0;
        let mut _last_review: Option<ReviewResult> = None;
        let mut iteration_context: Option<IterationContext> = None;

        while iteration < self.max_iterations {
            iteration += 1;
            info!("Starting iteration {}/{}", iteration, self.max_iterations);

            // Create or update iteration context
            let mut current_context = iteration_context
                .take()
                .unwrap_or_else(|| IterationContext::new(iteration));
            current_context.iteration = iteration;

            info!(
                "Starting iteration {} with {} existing files",
                iteration,
                current_context.existing_files.len()
            );
            for (filename, _) in &current_context.existing_files {
                info!("  Existing file: {}", filename);
            }

            // Emit iteration started event
            self.event_bus
                .emit(Event::Custom {
                    event_type: "iteration_started".to_string(),
                    data: serde_json::json!({
                        "iteration": iteration,
                        "max_iterations": self.max_iterations,
                        "has_existing_files": current_context.has_existing_files(),
                    }),
                })
                .await?;

            // Plan the task
            info!("Creating plan for task...");
            let plan = match self
                .planner
                .plan(
                    &task,
                    &*self.llm_manager,
                    self.config.as_deref(),
                    Some(&current_context),
                )
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    error!("Planning failed: {}", e);
                    self.emit_task_failed("Planning failed", &e.to_string())
                        .await?;
                    return Err(e);
                }
            };

            info!(
                "Plan created with {} steps, complexity: {:?}",
                plan.steps.len(),
                plan.estimated_complexity
            );

            // Execute the plan
            info!("Executing plan...");
            let results = match self.executor.execute(&plan, context_id).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Execution failed: {}", e);
                    self.emit_task_failed("Execution failed", &e.to_string())
                        .await?;
                    return Err(e);
                }
            };

            // Count successful steps
            let successful_steps = results.iter().filter(|r| r.success).count();
            info!(
                "Executed {}/{} steps successfully",
                successful_steps,
                results.len()
            );

            // Update iteration context with created artifacts
            if let Some(artifact_mgr) = &self.artifact_manager {
                let artifacts = artifact_mgr.list_artifacts().await;
                info!(
                    "Found {} artifacts to add to iteration context",
                    artifacts.len()
                );
                for artifact in artifacts {
                    let path = artifact.name.clone();
                    if !current_context.existing_files.contains_key(&path) {
                        info!("Adding artifact to iteration context: {}", path);
                        let file_info = FileInfo {
                            path: path.clone(),
                            language: match &artifact.artifact_type {
                                ArtifactType::SourceCode => "source",
                                ArtifactType::Configuration => "config",
                                ArtifactType::Documentation => "markdown",
                                ArtifactType::Test => "test",
                                ArtifactType::Build => "build",
                                ArtifactType::Script => "script",
                                ArtifactType::Data => "data",
                                ArtifactType::Other(_) => "other",
                            }
                            .to_string(),
                            description: artifact
                                .metadata
                                .get("description")
                                .cloned()
                                .unwrap_or_else(|| format!("{} file", artifact.artifact_type)),
                            has_issues: false,
                            issues: Vec::new(),
                        };
                        current_context.add_file(path, file_info);
                    }
                }
                info!(
                    "Iteration context now has {} files",
                    current_context.existing_files.len()
                );
            }

            // Review the results
            info!("Reviewing execution results...");
            let review = match self
                .reviewer
                .review(&plan, &results, &*self.llm_manager, context_id)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    error!("Review failed: {}", e);
                    self.emit_task_failed("Review failed", &e.to_string())
                        .await?;
                    return Err(e);
                }
            };

            info!("Review complete: {}", review.summary);

            // Log the actual issues found
            if !review.issues.is_empty() {
                info!("Issues found during review:");
                for issue in &review.issues {
                    info!(
                        "  - [{}] {:?}: {}",
                        issue.severity, issue.category, issue.description
                    );
                    if let Some(suggestion) = &issue.suggestion {
                        info!("    Suggestion: {}", suggestion);
                    }
                }
            }

            // Update iteration context with review results
            current_context.update_from_review(review.clone());
            current_context.progress_summary = format!(
                "Completed {} steps. Review: {}",
                successful_steps, review.summary
            );

            // Check if we're done
            if review.ready_to_deploy {
                info!("Task completed successfully!");

                // Post-process artifacts to clean up and organize
                if let Some(artifact_mgr) = &self.artifact_manager {
                    if let Err(e) = self.post_process_artifacts(artifact_mgr).await {
                        warn!("Failed to post-process artifacts: {}", e);
                    }
                }

                self.emit_task_completed(&plan, &results, &review).await?;
                return Ok(());
            }

            // Check if we should continue
            if iteration >= self.max_iterations {
                warn!("Max iterations reached without completing task");
                self.emit_task_failed(
                    "Max iterations reached",
                    &format!("Failed to complete task after {} iterations", iteration),
                )
                .await?;
                break;
            }

            // Handle critical issues
            let critical_issues = review
                .issues
                .iter()
                .filter(|i| i.severity == IssueSeverity::Critical)
                .count();

            if critical_issues > 0 {
                warn!(
                    "Found {} critical issues, will revise plan",
                    critical_issues
                );
            }

            // Store the context for the next iteration
            iteration_context = Some(current_context);
        }

        warn!("Exited loop without resolution");
        self.emit_task_failed(
            "Loop exited",
            "Agentic loop exited without completing the task",
        )
        .await?;

        Ok(())
    }

    async fn emit_task_completed(
        &self,
        plan: &Plan,
        results: &[StepResult],
        review: &ReviewResult,
    ) -> Result<()> {
        let artifacts: Vec<String> = results
            .iter()
            .flat_map(|r| r.artifacts_created.clone())
            .collect();

        self.event_bus.emit(Event::TaskCompleted {
            task_id: "main".to_string(),
            result: format!(
                "Task completed successfully. {} steps executed. Quality: {:?}. {} artifacts created.",
                results.len(),
                review.overall_quality,
                artifacts.len()
            ),
        }).await?;

        self.event_bus
            .emit(Event::Custom {
                event_type: "task_summary".to_string(),
                data: serde_json::json!({
                    "plan_goal": plan.goal,
                    "steps_executed": results.len(),
                    "steps_successful": results.iter().filter(|r| r.success).count(),
                    "artifacts_created": artifacts,
                    "quality": format!("{:?}", review.overall_quality),
                    "issues_found": review.issues.len(),
                    "suggestions": review.suggestions.len(),
                }),
            })
            .await?;

        Ok(())
    }

    async fn emit_task_failed(&self, reason: &str, details: &str) -> Result<()> {
        self.event_bus
            .emit(Event::TaskFailed {
                task_id: "main".to_string(),
                error: format!("{}: {}", reason, details),
            })
            .await?;
        Ok(())
    }

    /// Post-process artifacts to clean up duplicates and organize files
    async fn post_process_artifacts(&self, artifact_mgr: &Arc<ArtifactManager>) -> Result<()> {
        info!("Post-processing artifacts...");

        let artifacts = artifact_mgr.list_artifacts().await;

        // Count artifacts by type
        let mut artifact_stats: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut generic_artifacts = 0;

        for artifact in &artifacts {
            let filename = artifact.name.to_lowercase();

            // Count generic vs named artifacts
            if filename.starts_with("code_block_") || filename.starts_with("code_") {
                generic_artifacts += 1;
            }

            // Count by artifact type
            *artifact_stats
                .entry(artifact.artifact_type.to_string())
                .or_insert(0) += 1;
        }

        info!(
            "Post-processing complete. Found {} total artifacts ({} generic):",
            artifacts.len(),
            generic_artifacts
        );

        for (artifact_type, count) in artifact_stats {
            info!("  - {}: {}", artifact_type, count);
        }

        // TODO: In the future, we could:
        // - Detect duplicate content across files
        // - Merge related files that were split unnecessarily
        // - Rename generic files based on content analysis
        // - Clean up temporary or intermediate files
        // But this requires more sophisticated content analysis

        Ok(())
    }
}

// Note: EventEmitter trait implementation removed as AgenticLoop
// doesn't directly emit events, it uses the event_bus
