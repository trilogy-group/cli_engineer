use anyhow::Result;

/// Represents a parsed user task.
#[derive(Debug, Clone)]
pub struct Task {
    pub description: String,
}

/// Interprets raw input into a `Task`.
pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self { Self }

    /// Interpret user input into a `Task`.
    pub fn interpret(&self, input: &str) -> Result<Task> {
        // For now just wrap the input.
        Ok(Task { description: input.to_string() })
    }
}
