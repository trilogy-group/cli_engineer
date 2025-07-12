use crate::reviewer::{Issue, ReviewResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Context passed between iterations to maintain state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationContext {
    /// Current iteration number
    pub iteration: usize,

    /// Files created/modified in previous iterations
    pub existing_files: HashMap<String, FileInfo>,

    /// Review feedback from the last iteration
    pub last_review: Option<ReviewResult>,

    /// Issues that need to be addressed
    pub pending_issues: Vec<Issue>,

    /// Summary of what has been accomplished so far
    pub progress_summary: String,
    
    /// Command outputs from previous steps (for adaptive planning)
    pub command_outputs: Vec<CommandOutput>,
    
    /// Failed commands that may need retry
    pub failed_commands: Vec<FailedCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// Full path to the file
    pub path: String,

    /// Language/type of the file
    pub language: String,

    /// Brief description of the file's purpose
    pub description: String,

    /// Whether this file has known issues
    pub has_issues: bool,

    /// Specific issues with this file
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    /// The command that was executed
    pub command: String,
    
    /// The step ID this command was part of
    pub step_id: String,
    
    /// The output from the command
    pub output: String,
    
    /// Whether the command succeeded
    pub success: bool,
    
    /// Exit code if available
    pub exit_code: Option<i32>,
    
    /// When the command was executed
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedCommand {
    /// The command that failed
    pub command: String,
    
    /// The step ID this command was part of
    pub step_id: String,
    
    /// Error message or output
    pub error: String,
    
    /// Whether this is a build failure, test failure, etc
    pub failure_type: String,
    
    /// Number of times this has been retried
    pub retry_count: usize,
}

impl IterationContext {
    pub fn new(iteration: usize) -> Self {
        Self {
            iteration,
            existing_files: HashMap::new(),
            last_review: None,
            pending_issues: Vec::new(),
            progress_summary: String::new(),
            command_outputs: Vec::new(),
            failed_commands: Vec::new(),
        }
    }

    pub fn add_file(&mut self, filename: String, file_info: FileInfo) {
        self.existing_files.insert(filename, file_info);
    }

    pub fn update_from_review(&mut self, review: ReviewResult) {
        // Extract issues that need fixing
        self.pending_issues = review.issues.clone();

        // Mark files with issues
        for issue in &review.issues {
            if let Some(file) = issue.location.as_ref() {
                if let Some(file_info) = self.existing_files.get_mut(file) {
                    file_info.has_issues = true;
                    file_info.issues.push(issue.description.clone());
                }
            }
        }

        self.last_review = Some(review);
    }

    pub fn has_existing_files(&self) -> bool {
        !self.existing_files.is_empty()
    }
    
    pub fn add_command_output(&mut self, output: CommandOutput) {
        self.command_outputs.push(output);
    }
    
    pub fn add_failed_command(&mut self, failed: FailedCommand) {
        // Check if this command has already failed before
        if let Some(existing) = self.failed_commands.iter_mut()
            .find(|f| f.command == failed.command && f.step_id == failed.step_id) {
            existing.retry_count += 1;
            existing.error = failed.error; // Update with latest error
        } else {
            self.failed_commands.push(failed);
        }
    }
    
    pub fn has_failed_builds(&self) -> bool {
        self.failed_commands.iter().any(|f| f.failure_type == "build")
    }
    
    pub fn has_failed_tests(&self) -> bool {
        self.failed_commands.iter().any(|f| f.failure_type == "test")
    }
    
    pub fn get_recent_command_outputs(&self, limit: usize) -> Vec<&CommandOutput> {
        let start = self.command_outputs.len().saturating_sub(limit);
        self.command_outputs[start..].iter().collect()
    }
}

impl fmt::Display for IterationContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();

        // Basic info
        output.push_str(&format!("Iteration #{}\n", self.iteration));

        // Existing files
        if !self.existing_files.is_empty() {
            output.push_str("\nExisting files:\n");
            for (name, info) in &self.existing_files {
                output.push_str(&format!("  - {} ({})", name, info.language));
                if info.has_issues {
                    output.push_str(" [HAS ISSUES]");
                }
                output.push('\n');
                if !info.description.is_empty() {
                    output.push_str(&format!("    Description: {}\n", info.description));
                }
                for issue in &info.issues {
                    output.push_str(&format!("    Issue: {}\n", issue));
                }
            }
        }

        // Pending issues
        if !self.pending_issues.is_empty() {
            output.push_str(&format!(
                "\nPending issues ({}):\n",
                self.pending_issues.len()
            ));
            for issue in &self.pending_issues {
                output.push_str(&format!("  - {}: {}\n", issue.severity, issue.description));
            }
        }

        // Last review summary
        if let Some(review) = &self.last_review {
            output.push_str(&format!("\nLast review: {}\n", review.summary));
        }
        
        // Failed commands that need attention
        if !self.failed_commands.is_empty() {
            output.push_str(&format!("\nFailed commands ({}):\n", self.failed_commands.len()));
            for failed in &self.failed_commands {
                output.push_str(&format!("  - {} [{}] (step: {})\n", 
                    failed.command, failed.failure_type, failed.step_id));
                output.push_str(&format!("    Error: {}\n", failed.error));
                if failed.retry_count > 0 {
                    output.push_str(&format!("    Retry count: {}\n", failed.retry_count));
                }
            }
        }
        
        // Recent command outputs (last 5)
        let recent_outputs = self.get_recent_command_outputs(5);
        if !recent_outputs.is_empty() {
            output.push_str(&format!("\nRecent command outputs ({} total):\n", 
                self.command_outputs.len()));
            for cmd_output in recent_outputs {
                let status = if cmd_output.success { "✓" } else { "✗" };
                output.push_str(&format!("  {} {} (step: {})\n", 
                    status, cmd_output.command, cmd_output.step_id));
                // Show first 200 chars of output for context
                let truncated_output = if cmd_output.output.len() > 200 {
                    format!("{}...", &cmd_output.output[..200])
                } else {
                    cmd_output.output.clone()
                };
                // Only show output if it's meaningful (not just command echo)
                if !truncated_output.trim().is_empty() && 
                   !truncated_output.contains("Command:") {
                    output.push_str(&format!("    Output: {}\n", 
                        truncated_output.lines().next().unwrap_or("").trim()));
                }
            }
        }
        
        // Progress summary
        if !self.progress_summary.is_empty() {
            output.push_str(&format!("\nProgress: {}\n", self.progress_summary));
        }

        write!(f, "{}", output)
    }
}
