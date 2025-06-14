use anyhow::Result;

/// Represents a parsed user task.
#[derive(Debug, Clone)]
pub struct Task {
    pub description: String,
    pub goal: String,
}

/// Interprets raw input into a `Task`.
pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self {
        Self
    }

    /// Interpret user input into a `Task`.
    pub fn interpret(&self, input: &str) -> Result<Task> {
        // Extract goal from input - in production this would use NLP
        let goal = if input.contains("create") || input.contains("build") {
            format!("Create or build: {}", input)
        } else if input.contains("fix") || input.contains("debug") {
            format!("Fix or debug: {}", input)
        } else if input.contains("test") {
            format!("Test: {}", input)
        } else {
            format!("Complete task: {}", input)
        };

        Ok(Task {
            description: input.to_string(),
            goal,
        })
    }
}
