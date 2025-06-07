use crate::context::ContextManager;
use crate::event_bus::{Event, EventBus};
use crate::executor::StepResult;
use crate::llm_manager::LLMManager;
use crate::planner::Plan;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

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
    Excellent, // No issues, follows best practices
    Good,      // Minor issues or improvements possible
    Fair,      // Some issues that should be addressed
    Poor,      // Major issues requiring rework
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub description: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    Critical, // Must fix before proceeding
    Major,    // Should fix for quality
    Minor,    // Nice to fix but not blocking
    Info,     // Informational only
}

impl fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueSeverity::Critical => write!(f, "Critical"),
            IssueSeverity::Major => write!(f, "Major"),
            IssueSeverity::Minor => write!(f, "Minor"),
            IssueSeverity::Info => write!(f, "Info"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueCategory {
    Logic,         // Logic errors or bugs
    Performance,   // Performance concerns
    Security,      // Security vulnerabilities
    CodeStyle,     // Style and formatting
    BestPractices, // Not following best practices
    Documentation, // Missing or poor documentation
    Testing,       // Insufficient testing
    Dependencies,  // Dependency issues
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
            let _ = bus
                .emit(Event::Custom {
                    event_type: "review_started".to_string(),
                    data: serde_json::json!({
                        "plan_goal": plan.goal,
                        "steps_executed": results.len(),
                    }),
                })
                .await;
        }

        // Build review prompt
        let prompt = self.build_review_prompt(plan, results);

        // Add to context if available
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr
                .add_message(context_id, "user".to_string(), prompt.clone())
                .await?;
        }

        // Get review from LLM
        let response = llm_manager
            .send_prompt(&prompt)
            .await
            .context("Failed to get review response from LLM")?;

        // Add response to context
        if let Some(ctx_mgr) = &self.context_manager {
            ctx_mgr
                .add_message(context_id, "assistant".to_string(), response.clone())
                .await?;
        }

        // Parse review response
        let review_result = self
            .parse_review_response(&response, results)
            .context("Failed to parse review response")?;

        // Emit review completed event
        if let Some(bus) = &self.event_bus {
            let _ = bus
                .emit(Event::Custom {
                    event_type: "review_completed".to_string(),
                    data: serde_json::json!({
                        "quality": format!("{:?}", review_result.overall_quality),
                        "issues_count": review_result.issues.len(),
                        "ready_to_deploy": review_result.ready_to_deploy,
                    }),
                })
                .await;
        }

        Ok(review_result)
    }

    fn build_review_prompt(&self, plan: &Plan, results: &[StepResult]) -> String {
        let mut outputs_summary = String::new();

        // Check if this is a documentation task
        let is_documentation_task = plan.goal.to_lowercase().contains("documentation") || 
                                   plan.goal.to_lowercase().contains("docs");

        // Collect all created artifacts for documentation-specific checks
        let mut all_artifacts = Vec::new();

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
                outputs_summary.push_str(&format!(
                    "Artifacts created: {:?}\n",
                    result.artifacts_created
                ));
                all_artifacts.extend(result.artifacts_created.clone());
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

        // Build the base prompt
        let mut prompt = format!(
            "{}\n\nPlan Goal: {}\nTotal Steps: {}\n\nExecution Results:{}\n\n",
            self.review_prompt_template,
            plan.goal,
            plan.steps.len(),
            outputs_summary
        );

        // Add documentation-specific review criteria if applicable
        if is_documentation_task {
            prompt.push_str("\n### DOCUMENTATION-SPECIFIC REVIEW CRITERIA ###\n");
            prompt.push_str("Please pay special attention to these documentation-specific issues:\n\n");
            
            prompt.push_str("1. **File Organization**:\n");
            prompt.push_str("   - Check that ALL documentation files are in the artifacts/docs/ directory\n");
            prompt.push_str("   - Flag any files created outside of artifacts/docs/ as CRITICAL issues\n");
            prompt.push_str("   - Non-documentation files (code, configs, etc.) should NOT be in artifacts\n\n");
            
            prompt.push_str("2. **File Completeness**:\n");
            prompt.push_str("   - Check each file for incomplete content (e.g., sections that say 'TODO' or appear cut off)\n");
            prompt.push_str("   - Check for files that end abruptly without proper conclusion\n");
            prompt.push_str("   - Look for files with fewer than 30 lines that seem to be just stubs or introductions\n");
            prompt.push_str("   - Check for headings without content (e.g., '## Section' followed by nothing)\n");
            prompt.push_str("   - Check for incomplete sentences or paragraphs that seem cut off mid-thought\n");
            prompt.push_str("   - Flag incomplete files as MAJOR issues\n\n");
            
            prompt.push_str("3. **Link Integrity**:\n");
            prompt.push_str("   - Check all internal links (e.g., [text](filename.md)) reference files that actually exist\n");
            prompt.push_str("   - Flag broken links as MAJOR issues and list the missing files\n");
            prompt.push_str("   - Suggest creating the missing files in the next iteration\n\n");
            
            prompt.push_str("4. **Content Quality**:\n");
            prompt.push_str("   - Ensure documentation is specific to the actual codebase, not generic\n");
            prompt.push_str("   - Check that API documentation matches actual code structure\n\n");
            
            if !all_artifacts.is_empty() {
                prompt.push_str(&format!("\nFiles created in this execution:\n{}\n", 
                    all_artifacts.join("\n")));
            }
        }

        prompt.push_str("\nProvide a comprehensive review following the format specified above.");
        
        prompt
    }

    fn parse_review_response(
        &self,
        response: &str,
        _results: &[StepResult],
    ) -> Result<ReviewResult> {
        let mut overall_quality = QualityLevel::Good;
        let mut ready_to_deploy = false;
        let mut summary = String::new();
        let mut issues = Vec::new();

        // Parse structured response
        let lines: Vec<&str> = response.lines().collect();
        let mut in_issues_section = false;

        for line in lines {
            let line = line.trim();

            if line.starts_with("QUALITY:") {
                let quality_str = line.replace("QUALITY:", "").trim().to_lowercase();
                overall_quality = match quality_str.as_str() {
                    "excellent" => QualityLevel::Excellent,
                    "good" => QualityLevel::Good,
                    "fair" => QualityLevel::Fair,
                    "poor" => QualityLevel::Poor,
                    _ => QualityLevel::Good,
                };
            } else if line.starts_with("READY_TO_DEPLOY:") {
                ready_to_deploy = line.to_lowercase().contains("yes");
            } else if line.starts_with("SUMMARY:") {
                summary = line.replace("SUMMARY:", "").trim().to_string();
            } else if line.starts_with("ISSUES:") {
                in_issues_section = true;
            } else if in_issues_section && line.starts_with("- SEVERITY:") {
                // Parse issue line
                if let Some(issue) = self.parse_issue_line(line) {
                    issues.push(issue);
                }
            }
        }

        // Fallback summary if not found
        if summary.is_empty() {
            let issue_count = issues.len();
            let critical_count = issues
                .iter()
                .filter(|i| matches!(i.severity, IssueSeverity::Critical))
                .count();
            
            // Auto-determine ready_to_deploy if not explicitly set by LLM
            // Ready to deploy if quality is good/excellent AND no critical issues
            if !ready_to_deploy {
                ready_to_deploy = matches!(overall_quality, QualityLevel::Good | QualityLevel::Excellent) 
                    && critical_count == 0;
            }
            
            summary = format!(
                "Review complete. Quality: {:?}. Found {} issues ({} critical). {}",
                overall_quality,
                issue_count,
                critical_count,
                if ready_to_deploy {
                    "Ready to deploy"
                } else {
                    "Not ready to deploy"
                }
            );
        }

        Ok(ReviewResult {
            overall_quality,
            issues,
            suggestions: Vec::new(),
            ready_to_deploy,
            summary,
        })
    }

    fn parse_issue_line(&self, line: &str) -> Option<Issue> {
        // Remove the leading "- SEVERITY: " part
        let content = line.strip_prefix("- SEVERITY:")?.trim();

        // Split by "|" to get parts
        let parts: Vec<&str> = content.split("|").collect();

        if parts.len() < 4 {
            return None;
        }

        // Extract severity from first part
        let severity_str = parts[0].trim().to_lowercase();
        let severity = match severity_str.as_str() {
            "critical" => IssueSeverity::Critical,
            "major" => IssueSeverity::Major,
            "minor" => IssueSeverity::Minor,
            "suggestion" => IssueSeverity::Info,
            _ => return None,
        };

        // Extract category from "CATEGORY: xxx" format
        let category_part = parts[1].trim();
        let category_str = category_part
            .strip_prefix("CATEGORY:")?
            .trim()
            .to_lowercase();
        let category = match category_str.as_str() {
            "logic" => IssueCategory::Logic,
            "performance" => IssueCategory::Performance,
            "security" => IssueCategory::Security,
            "codestyle" => IssueCategory::CodeStyle,
            "bestpractices" => IssueCategory::BestPractices,
            "documentation" => IssueCategory::Documentation,
            "testing" => IssueCategory::Testing,
            "dependencies" => IssueCategory::Dependencies,
            _ => return None,
        };

        // Extract description
        let desc_part = parts[2].trim();
        let description = desc_part.strip_prefix("DESCRIPTION:")?.trim().to_string();

        // Extract suggestion
        let suggestion = if parts.len() > 3 {
            let sug_part = parts[3].trim();
            sug_part
                .strip_prefix("SUGGESTION:")
                .map(|s| s.trim().to_string())
        } else {
            None
        };

        Some(Issue {
            severity,
            category,
            description,
            location: None,
            suggestion,
        })
    }

    fn default_review_prompt() -> String {
        r#"You are a senior software engineer conducting a code review.

Review the execution results and identify ACTUAL issues if any exist.

IMPORTANT: Only report issues that ACTUALLY exist in the code. Do not report theoretical or potential issues that don't apply to the specific code.

For each ACTUAL issue found, specify:
- Severity: Critical (blocks functionality), Major (significant problem), Minor (small issue), Suggestion (improvement)
- Category: Logic, Security, Performance, CodeStyle, BestPractices, Documentation, Testing
- Description: Specific description of the actual issue
- Location: Where the issue is (if applicable)
- Suggestion: How to fix it

Format your response as:
QUALITY: [Excellent/Good/Fair/Poor]
READY_TO_DEPLOY: [Yes/No]
SUMMARY: [One line summary]

ISSUES:
[If no issues exist, write "No issues found"]
[Otherwise list each issue as:]
- SEVERITY: [severity] | CATEGORY: [category] | DESCRIPTION: [description] | SUGGESTION: [suggestion]

Be honest and accurate. For simple scripts like "Hello World", there are usually NO actual issues."#.to_string()
    }
}

impl Default for Reviewer {
    fn default() -> Self {
        Self::new()
    }
}
