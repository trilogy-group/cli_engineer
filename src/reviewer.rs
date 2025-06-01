use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::executor::StepResult;
use crate::planner::Plan;
use crate::context::ContextManager;
use crate::event_bus::{EventBus, Event};
use crate::llm_manager::LLMManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub overall_quality: QualityLevel,
    pub issues: Vec<Issue>,
    pub suggestions: Vec<Suggestion>,
    pub ready_to_deploy: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityLevel {
    Excellent,  // No issues, follows best practices
    Good,       // Minor issues or improvements possible
    Fair,       // Some issues that should be addressed
    Poor,       // Major issues requiring rework
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub description: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Critical,  // Must fix before proceeding
    Major,     // Should fix for quality
    Minor,     // Nice to fix but not blocking
    Info,      // Informational only
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueCategory {
    Logic,           // Logic errors or bugs
    Performance,     // Performance concerns
    Security,        // Security vulnerabilities
    CodeStyle,       // Style and formatting
    BestPractices,   // Not following best practices
    Documentation,   // Missing or poor documentation
    Testing,         // Insufficient testing
    Dependencies,    // Dependency issues
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub title: String,
    pub description: String,
    pub priority: SuggestionPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionPriority {
    High,
    Medium,
    Low,
}

pub struct Reviewer {
    context_manager: Option<Arc<ContextManager>>,
    event_bus: Option<Arc<EventBus>>,
    review_prompt_template: String,
}

impl Reviewer {
    pub fn new() -> Self {
        Self {
            context_manager: None,
            event_bus: None,
            review_prompt_template: Self::default_review_prompt(),
        }
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

    /// Review the execution results for correctness and quality
    pub async fn review(
        &self,
        plan: &Plan,
        results: &[StepResult],
        llm_manager: &LLMManager,
        context_id: &str,
    ) -> Result<ReviewResult> {
        // Emit review started event
        if let Some(bus) = &self.event_bus {
            let _ = bus.emit(Event::Custom {
                event_type: "review_started".to_string(),
                data: serde_json::json!({
                    "plan_goal": plan.goal,
                    "steps_executed": results.len(),
                }),
            }).await;
        }

        // Build review prompt
        let prompt = self.build_review_prompt(plan, results);
        
        // Add to context if available
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr.add_message(context_id, "user".to_string(), prompt.clone()).await?;
        }
        
        // Get review from LLM
        let response = llm_manager.send_prompt(&prompt).await
            .context("Failed to get review response from LLM")?;
        
        // Add response to context
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr.add_message(context_id, "assistant".to_string(), response.clone()).await?;
        }
        
        // Parse review response
        let review_result = self.parse_review_response(&response, results)
            .context("Failed to parse review response")?;
        
        // Emit review completed event
        if let Some(bus) = &self.event_bus {
            let _ = bus.emit(Event::Custom {
                event_type: "review_completed".to_string(),
                data: serde_json::json!({
                    "quality": format!("{:?}", review_result.overall_quality),
                    "issues_count": review_result.issues.len(),
                    "ready_to_deploy": review_result.ready_to_deploy,
                }),
            }).await;
        }
        
        Ok(review_result)
    }

    fn build_review_prompt(&self, plan: &Plan, results: &[StepResult]) -> String {
        let mut outputs_summary = String::new();
        
        for (i, result) in results.iter().enumerate() {
            outputs_summary.push_str(&format!(
                "\n--- Step {} ({}) ---\n",
                i + 1,
                if result.success { "SUCCESS" } else { "FAILED" }
            ));
            
            if let Some(step) = plan.steps.iter().find(|s| s.id == result.step_id) {
                outputs_summary.push_str(&format!("Description: {}\n", step.description));
                outputs_summary.push_str(&format!("Category: {:?}\n", step.category));
            }
            
            if !result.artifacts_created.is_empty() {
                outputs_summary.push_str(&format!("Artifacts created: {:?}\n", result.artifacts_created));
            }
            
            if let Some(error) = &result.error {
                outputs_summary.push_str(&format!("Error: {}\n", error));
            } else {
                // Truncate very long outputs
                let output = if result.output.len() > 1000 {
                    format!("{}... (truncated)", &result.output[..1000])
                } else {
                    result.output.clone()
                };
                outputs_summary.push_str(&format!("Output:\n{}\n", output));
            }
        }
        
        format!(
            "{}\n\nPlan Goal: {}\nTotal Steps: {}\n\nExecution Results:{}\n\nProvide a comprehensive review.",
            self.review_prompt_template,
            plan.goal,
            plan.steps.len(),
            outputs_summary
        )
    }

    fn parse_review_response(&self, response: &str, results: &[StepResult]) -> Result<ReviewResult> {
        // Parse the LLM response to extract structured review
        // In production, this would use more sophisticated parsing or ask for JSON
        
        let lower_response = response.to_lowercase();
        
        // Determine overall quality based on keywords
        let overall_quality = if lower_response.contains("excellent") || lower_response.contains("perfect") {
            QualityLevel::Excellent
        } else if lower_response.contains("good") || lower_response.contains("well") {
            QualityLevel::Good
        } else if lower_response.contains("fair") || lower_response.contains("adequate") {
            QualityLevel::Fair
        } else if lower_response.contains("poor") || lower_response.contains("bad") {
            QualityLevel::Poor
        } else {
            QualityLevel::Good // Default
        };
        
        // Extract issues based on keywords
        let mut issues = Vec::new();
        
        if lower_response.contains("error") || lower_response.contains("bug") {
            issues.push(Issue {
                severity: IssueSeverity::Major,
                category: IssueCategory::Logic,
                description: "Potential logic error detected".to_string(),
                location: None,
                suggestion: Some("Review logic for correctness".to_string()),
            });
        }
        
        if lower_response.contains("security") || lower_response.contains("vulnerable") {
            issues.push(Issue {
                severity: IssueSeverity::Critical,
                category: IssueCategory::Security,
                description: "Security concern identified".to_string(),
                location: None,
                suggestion: Some("Address security vulnerabilities".to_string()),
            });
        }
        
        if lower_response.contains("performance") || lower_response.contains("slow") {
            issues.push(Issue {
                severity: IssueSeverity::Minor,
                category: IssueCategory::Performance,
                description: "Performance optimization opportunity".to_string(),
                location: None,
                suggestion: Some("Consider performance improvements".to_string()),
            });
        }
        
        // Check for failed steps
        for result in results {
            if !result.success {
                issues.push(Issue {
                    severity: IssueSeverity::Critical,
                    category: IssueCategory::Logic,
                    description: format!("Step {} failed", result.step_id),
                    location: Some(result.step_id.clone()),
                    suggestion: result.error.clone(),
                });
            }
        }
        
        // Generate suggestions
        let mut suggestions = Vec::new();
        
        if !lower_response.contains("test") {
            suggestions.push(Suggestion {
                title: "Add Tests".to_string(),
                description: "Consider adding unit and integration tests".to_string(),
                priority: SuggestionPriority::High,
            });
        }
        
        if !lower_response.contains("document") {
            suggestions.push(Suggestion {
                title: "Improve Documentation".to_string(),
                description: "Add or improve code documentation".to_string(),
                priority: SuggestionPriority::Medium,
            });
        }
        
        // Determine if ready to deploy
        let critical_issues = issues.iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Critical))
            .count();
        
        let ready_to_deploy = critical_issues == 0 && 
            !matches!(overall_quality, QualityLevel::Poor);
        
        // Generate summary
        let summary = format!(
            "Review complete. Quality: {:?}. Found {} issues ({} critical). {}",
            overall_quality,
            issues.len(),
            critical_issues,
            if ready_to_deploy { "Ready to deploy." } else { "Not ready to deploy." }
        );
        
        Ok(ReviewResult {
            overall_quality,
            issues,
            suggestions,
            ready_to_deploy,
            summary,
        })
    }

    fn default_review_prompt() -> String {
        r#"You are a senior software engineer conducting a thorough code review.

Review the execution results below for:
1. Correctness - Does the implementation meet the stated goal?
2. Code Quality - Is the code clean, readable, and maintainable?
3. Best Practices - Does it follow language-specific best practices?
4. Security - Are there any security vulnerabilities?
5. Performance - Are there obvious performance issues?
6. Testing - Is the code adequately tested?
7. Documentation - Is the code well-documented?

Identify any issues, categorize them by severity, and provide constructive suggestions.
Be specific about problems and how to fix them."#.to_string()
    }
}

impl Default for Reviewer {
    fn default() -> Self {
        Self::new()
    }
}
