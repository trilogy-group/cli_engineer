use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Main configuration structure for cli_engineer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AI provider configurations
    pub ai_providers: AIProvidersConfig,

    /// Task execution configuration
    pub execution: ExecutionConfig,

    /// UI display configuration
    pub ui: UIConfig,

    /// Context management configuration
    pub context: ContextConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProvidersConfig {
    /// OpenAI configuration
    pub openai: Option<ProviderConfig>,

    /// Anthropic configuration
    pub anthropic: Option<ProviderConfig>,

    /// OpenRouter configuration
    pub openrouter: Option<ProviderConfig>,

    /// Gemini configuration
    pub gemini: Option<ProviderConfig>,

    /// Ollama configuration
    pub ollama: Option<OllamaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled
    pub enabled: bool,

    /// Model to use
    pub model: String,

    /// Temperature setting
    pub temperature: Option<f32>,

    /// Cost per 1M input tokens (in USD)
    pub cost_per_1m_input_tokens: Option<f32>,

    /// Cost per 1M output tokens (in USD)
    pub cost_per_1m_output_tokens: Option<f32>,

    /// Maximum context size in tokens
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Whether this provider is enabled
    pub enabled: bool,

    /// Model to use
    pub model: String,

    /// Temperature setting
    pub temperature: Option<f32>,

    /// Base URL for Ollama server
    pub base_url: Option<String>,

    /// Maximum context size in tokens
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Maximum iterations for the agentic loop
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    /// Enable parallel task execution
    #[serde(default = "default_parallel_enabled")]
    pub parallel_enabled: bool,

    /// Working directory for artifacts
    #[serde(default = "default_artifact_dir")]
    pub artifact_dir: String,

    /// Enable isolated execution environments
    #[serde(default = "default_isolated_execution")]
    pub isolated_execution: bool,

    /// Clean up artifacts on exit
    #[serde(default = "default_cleanup_on_exit")]
    pub cleanup_on_exit: bool,

    /// Disable automatic git repository initialization unless explicitly requested
    #[serde(default = "default_disable_auto_git")]
    pub disable_auto_git: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    /// Enable colorful output
    #[serde(default = "default_colorful")]
    pub colorful: bool,

    /// Show progress bars
    #[serde(default = "default_progress_bars")]
    pub progress_bars: bool,

    /// Show real-time metrics
    #[serde(default = "default_metrics")]
    pub metrics: bool,

    /// Output format ("terminal", "json", "plain")
    #[serde(default = "default_output_format")]
    pub output_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Fallback maximum tokens for context (only used if LLM manager unavailable)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Compression threshold (0.0 to 1.0)
    #[serde(default = "default_compression_threshold")]
    pub compression_threshold: f32,

    /// Enable context caching
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
}

// Default value functions
fn default_max_iterations() -> usize {
    10
}
fn default_parallel_enabled() -> bool {
    false
}
fn default_artifact_dir() -> String {
    "./artifacts".to_string()
}
fn default_isolated_execution() -> bool {
    false
}
fn default_cleanup_on_exit() -> bool {
    false
}
fn default_colorful() -> bool {
    true
}
fn default_progress_bars() -> bool {
    true
}
fn default_metrics() -> bool {
    true
}
fn default_output_format() -> String {
    "terminal".to_string()
}
fn default_max_tokens() -> usize {
    100_000
}
fn default_compression_threshold() -> f32 {
    0.8
}
fn default_cache_enabled() -> bool {
    true
}
fn default_disable_auto_git() -> bool {
    false
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ai_providers: AIProvidersConfig {
                openai: Some(ProviderConfig {
                    enabled: true,
                    model: "o4-mini".to_string(),
                    temperature: Some(1.0), // OpenAI o4-mini only supports temperature 1.0
                    cost_per_1m_input_tokens: None,
                    cost_per_1m_output_tokens: None,
                    max_tokens: None,
                }),
                anthropic: Some(ProviderConfig {
                    enabled: false,
                    model: "claude-sonnet-4-0".to_string(),
                    temperature: Some(0.7),
                    cost_per_1m_input_tokens: None,
                    cost_per_1m_output_tokens: None,
                    max_tokens: None,
                }),
                openrouter: Some(ProviderConfig {
                    enabled: false,
                    model: "deepseek/deepseek-r1-0528-qwen3-8b".to_string(),
                    temperature: Some(0.2),
                    cost_per_1m_input_tokens: None,
                    cost_per_1m_output_tokens: None,
                    max_tokens: None,
                }),
                gemini: Some(ProviderConfig {
                    enabled: false,
                    model: "gemini-1.5-flash-latest".to_string(),
                    temperature: Some(0.2),
                    cost_per_1m_input_tokens: None,
                    cost_per_1m_output_tokens: None,
                    max_tokens: None,
                }),
                ollama: Some(OllamaConfig {
                    enabled: false,
                    model: "qwen3:8b".to_string(),
                    temperature: Some(0.7),
                    base_url: Some("http://localhost:11434".to_string()),
                    max_tokens: Some(8192),
                }),
            },
            execution: ExecutionConfig {
                max_iterations: default_max_iterations(),
                parallel_enabled: default_parallel_enabled(),
                artifact_dir: default_artifact_dir(),
                isolated_execution: default_isolated_execution(),
                cleanup_on_exit: default_cleanup_on_exit(),
                disable_auto_git: default_disable_auto_git(),
            },
            ui: UIConfig {
                colorful: default_colorful(),
                progress_bars: default_progress_bars(),
                metrics: default_metrics(),
                output_format: default_output_format(),
            },
            context: ContextConfig {
                max_tokens: default_max_tokens(),
                compression_threshold: default_compression_threshold(),
                cache_enabled: default_cache_enabled(),
            },
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.as_ref().display()))
    }

    /// Load configuration from command line argument or default locations
    pub fn load(config_path: &Option<String>) -> Result<Self> {
        if let Some(path) = config_path {
            return Self::from_file(path);
        }

        // Try loading from default locations
        let default_paths = vec![
            "cli_engineer.toml",
            ".cli_engineer.toml",
            "~/.config/cli_engineer/config.toml",
        ];

        for path in default_paths {
            let expanded_path = shellexpand::tilde(path);
            if Path::new(expanded_path.as_ref()).exists() {
                match Self::from_file(expanded_path.as_ref()) {
                    Ok(config) => return Ok(config),
                    Err(e) => eprintln!("Warning: Failed to load config from {}: {}", path, e),
                }
            }
        }

        // Return default config if no file found
        Ok(Self::default())
    }

    /// Save configuration to a file
    #[allow(dead_code)]
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let contents = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        fs::write(path.as_ref(), contents)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;

        Ok(())
    }

    /// Merge with command-line arguments (CLI args take precedence)
    #[allow(dead_code)]
    pub fn merge_with_args(&mut self, headless: bool, _verbose: bool) {
        if headless {
            self.ui.colorful = false;
            self.ui.progress_bars = false;
            self.ui.metrics = false;
        }
    }
}
