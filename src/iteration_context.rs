use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use crate::reviewer::{ReviewResult, Issue};

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

impl IterationContext {
    pub fn new(iteration: usize) -> Self {
        Self {
            iteration,
            existing_files: HashMap::new(),
            last_review: None,
            pending_issues: Vec::new(),
            progress_summary: String::new(),
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
            output.push_str(&format!("\nPending issues ({}):\n", self.pending_issues.len()));
            for issue in &self.pending_issues {
                output.push_str(&format!("  - {}: {}\n", issue.severity, issue.description));
            }
        }
        
        // Last review summary
        if let Some(review) = &self.last_review {
            output.push_str(&format!("\nLast review: {}\n", review.summary));
        }
        
        write!(f, "{}", output)
    }
}
