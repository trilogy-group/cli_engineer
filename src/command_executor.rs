use anyhow::{Context, Result};
use log::{info, warn};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::config::Config;

/// Executes shell commands with allowlist validation
pub struct CommandExecutor {
    config: Arc<Config>,
}

impl CommandExecutor {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Check if a command is allowed based on the allowlist
    pub fn is_command_allowed(&self, command: &str) -> bool {
        if !self.config.execution.enable_code_execution {
            return false;
        }

        let command_lower = command.trim().to_lowercase();
        
        // Check if the command starts with any allowed prefix
        self.config.execution.allowed_commands.iter().any(|allowed| {
            let allowed_lower = allowed.to_lowercase();
            command_lower.starts_with(&allowed_lower)
        })
    }

    /// Execute a command if it's allowed
    pub async fn execute_command(&self, command: &str, working_dir: Option<&str>) -> Result<CommandResult> {
        if !self.is_command_allowed(command) {
            return Err(anyhow::anyhow!(
                "Command '{}' is not allowed. Check your allowlist in cli_engineer.toml",
                command
            ));
        }

        info!("Executing allowed command: {}", command);

        // Parse the command into parts
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        let program = parts[0];
        let args = &parts[1..];

        // Create the command
        let mut cmd = TokioCommand::new(program);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Set working directory if provided
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Execute with timeout (30 seconds)
        let timeout_duration = Duration::from_secs(30);
        let result = timeout(timeout_duration, cmd.output()).await
            .context("Command execution timed out after 30 seconds")?
            .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let success = result.status.success();
        let exit_code = result.status.code().unwrap_or(-1);

        let cmd_result = CommandResult {
            command: command.to_string(),
            stdout,
            stderr,
            exit_code,
            success,
        };

        if !success {
            warn!("Command '{}' failed with exit code {}", cmd_result.command(), exit_code);
        }

        Ok(cmd_result)
    }

    /// Get the list of allowed commands for display
    pub fn get_allowed_commands(&self) -> &[String] {
        &self.config.execution.allowed_commands
    }
}

/// Result of command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

impl CommandResult {
    /// Get a formatted output string for display
    pub fn get_output(&self) -> String {
        let mut output = String::new();
        
        // Include command and success status
        output.push_str(&format!("Command: {}\n", self.command));
        output.push_str(&format!("Status: {}\n", if self.success { "SUCCESS" } else { "FAILED" }));
        output.push_str(&format!("Exit Code: {}\n", self.exit_code));
        
        if !self.stdout.is_empty() {
            output.push_str(&format!("\nSTDOUT:\n{}\n", self.stdout));
        }
        
        if !self.stderr.is_empty() {
            output.push_str(&format!("\nSTDERR:\n{}\n", self.stderr));
        }
        
        output
    }
    
    /// Get the command that was executed
    pub fn command(&self) -> &str {
        &self.command
    }
    
    /// Check if the command was successful
    pub fn is_success(&self) -> bool {
        self.success
    }
    
    /// Get a brief summary of the result
    pub fn summary(&self) -> String {
        format!(
            "Command '{}' {} (exit code: {})",
            self.command,
            if self.success { "succeeded" } else { "failed" },
            self.exit_code
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ExecutionConfig};

    #[test]
    fn test_command_allowlist() {
        let config = Arc::new(Config {
            execution: ExecutionConfig {
                enable_code_execution: true,
                allowed_commands: vec![
                    "ls".to_string(),
                    "git status".to_string(),
                    "cargo build".to_string(),
                ],
                ..Default::default()
            },
            ..Default::default()
        });

        let executor = CommandExecutor::new(config);

        // Test allowed commands
        assert!(executor.is_command_allowed("ls"));
        assert!(executor.is_command_allowed("ls -la"));
        assert!(executor.is_command_allowed("git status"));
        assert!(executor.is_command_allowed("cargo build --release"));

        // Test disallowed commands
        assert!(!executor.is_command_allowed("rm -rf /"));
        assert!(!executor.is_command_allowed("sudo rm"));
        assert!(!executor.is_command_allowed("curl evil.com"));
    }

    #[test]
    fn test_command_disabled() {
        let config = Arc::new(Config {
            execution: ExecutionConfig {
                enable_code_execution: false,
                allowed_commands: vec!["ls".to_string()],
                ..Default::default()
            },
            ..Default::default()
        });

        let executor = CommandExecutor::new(config);
        assert!(!executor.is_command_allowed("ls"));
    }

    #[test]
    fn test_command_result_api() {
        let result = CommandResult {
            command: "echo hello".to_string(),
            stdout: "hello\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
            success: true,
        };

        // Test individual field access
        assert_eq!(result.command(), "echo hello");
        assert!(result.is_success());
        assert_eq!(result.exit_code, 0);
        
        // Test formatted output
        let output = result.get_output();
        assert!(output.contains("Command: echo hello"));
        assert!(output.contains("Status: SUCCESS"));
        assert!(output.contains("Exit Code: 0"));
        assert!(output.contains("hello"));
        
        // Test summary
        let summary = result.summary();
        assert!(summary.contains("echo hello"));
        assert!(summary.contains("succeeded"));
    }
}
