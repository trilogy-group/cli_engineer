use anyhow::Result;
use clap::{Parser, ValueEnum};
use log::{error, info, warn};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::time::Duration;
use uuid::Uuid;
use walkdir::WalkDir;

use agentic_loop::AgenticLoop;
use artifact::ArtifactManager;
use config::Config;
use context::{ContextConfig, ContextManager};
use event_bus::{Event, EventBus, EventEmitter};
use llm_manager::{LLMManager, LLMProvider, LocalProvider};
use providers::{
    anthropic::AnthropicProvider, openai::OpenAIProvider, openrouter::OpenRouterProvider,
};
use ui_dashboard::DashboardUI;
use ui_enhanced::EnhancedUI;
mod logger_dashboard;

mod agentic_loop;
mod artifact;
mod concurrency;
mod config;
mod context;
mod event_bus;
mod executor;
mod interpreter;
mod iteration_context;
mod llm_manager;
mod logger;
mod planner;
mod providers;
mod reviewer;
mod ui_dashboard;
mod ui_enhanced;

#[derive(ValueEnum, Debug, Clone)]
enum CommandKind {
    #[clap(help = "Code generation")]
    Code,
    #[clap(help = "Refactoring")]
    Refactor,
    #[clap(help = "Code review")]
    Review,
    #[clap(help = "Documentation generation")]
    Docs,
    #[clap(help = "Security analysis")]
    Security,
}

#[derive(Parser, Debug)]
#[command(
    name = "cli_engineer",
    about = "Agentic CLI for software engineering automation"
)]
struct Args {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    /// Use dashboard UI (compact, non-scrolling display)
    #[arg(short, long)]
    dashboard: bool,
    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
    /// Command to execute
    #[arg(value_enum)]
    command: CommandKind,
    /// Optional prompt describing the task
    #[arg(last = true)]
    prompt: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Parse command line arguments
    let args = Args::parse();

    // Create event bus
    let event_bus = Arc::new(EventBus::new(1000));

    // Initialize logger
    if args.dashboard {
        let level = if args.verbose {
            log::LevelFilter::Info
        } else {
            log::LevelFilter::Warn
        };
        logger_dashboard::DashboardLogger::init(event_bus.clone(), level)
            .expect("Failed to init DashboardLogger");
    } else {
        logger::init(args.verbose);
    }

    // Load configuration
    let config = Arc::new(Config::load(&args.config)?);

    let prompt = args.prompt.join(" ");

    if args.dashboard {
        // Use dashboard UI when --dashboard is specified
        let mut ui = DashboardUI::new(false);
        ui.set_event_bus(event_bus.clone());

        // Start UI
        ui.start()?;

        if matches!(args.command, CommandKind::Code) && prompt.is_empty() {
            ui.display_error("PROMPT required for code command")?;
            ui.finish()?;
            return Ok(());
        }

        let ui_ref = Arc::new(Mutex::new(ui));
        let ui_clone = ui_ref.clone();
        let (stop_tx, mut stop_rx) = oneshot::channel();

        // Start periodic UI updates
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Ok(mut ui_guard) = ui_clone.try_lock() {
                            let _ = ui_guard.throttled_render();
                        }
                    }
                    _ = &mut stop_rx => break,
                }
            }
        });

        let result = match args.command {
            CommandKind::Code => run_with_ui(prompt.clone(), config.clone(), event_bus.clone(), false, args.command).await,
            CommandKind::Refactor => {
                let p = if prompt.is_empty() {
                    "Analyze the current directory and perform recommended refactoring.".to_string()
                } else {
                    prompt.clone()
                };
                run_with_ui(
                    format!("Refactor codebase. {}", p),
                    config.clone(),
                    event_bus.clone(),
                    true,
                    args.command,
                )
                .await
            }
            CommandKind::Review => {
                let p = if prompt.is_empty() {
                    "ANALYSIS ONLY: Review the codebase files and create a comprehensive code review report. DO NOT generate, modify, or create any source code files. ONLY analyze existing code and document your findings, suggestions, and recommendations in code_review.md. Focus on code quality, best practices, potential issues, and improvement opportunities.".to_string()
                } else {
                    format!("ANALYSIS ONLY: Review the codebase with focus on: {}. DO NOT generate, modify, or create any source code files. ONLY analyze existing code and document your findings in code_review.md", prompt)
                };
                run_with_ui(p, config.clone(), event_bus.clone(), true, args.command).await
            }
            CommandKind::Docs => {
                let p = if prompt.is_empty() {
                    "Generate comprehensive documentation for the codebase. Create documentation files in a docs/ directory.".to_string()
                } else {
                    format!("Generate documentation for the codebase with these instructions: {}. Create documentation files in a docs/ directory.", prompt)
                };
                run_with_ui(p, config.clone(), event_bus.clone(), true, args.command).await
            }
            CommandKind::Security => {
                let p = if prompt.is_empty() {
                    "SECURITY ANALYSIS ONLY: Perform a comprehensive security analysis of the codebase. DO NOT generate, modify, or create any source code files. ONLY analyze existing code for vulnerabilities, security issues, and best practice violations. Document your findings, risk assessments, and security recommendations in security_report.md.".to_string()
                } else {
                    format!("SECURITY ANALYSIS ONLY: Perform a security analysis of the codebase focusing on: {}. DO NOT generate, modify, or create any source code files. ONLY analyze existing code and document your security findings in security_report.md", prompt)
                };
                run_with_ui(p, config.clone(), event_bus.clone(), true, args.command).await
            }
        };

        match result {
            Ok(_) => {
                let _ = stop_tx.send(());
                let _ = handle.await;
                if let Ok(mut ui_guard) = ui_ref.try_lock() {
                    ui_guard.finish()?;
                }
            }
            Err(e) => {
                let _ = stop_tx.send(());
                let _ = handle.await;
                if let Ok(mut ui_guard) = ui_ref.try_lock() {
                    ui_guard.display_error(&format!("{}", e))?;
                    ui_guard.finish()?;
                }
                return Err(e);
            }
        }
    } else {
        // Use enhanced UI for verbose mode or when dashboard is not requested
        let mut ui = if config.ui.colorful && config.ui.progress_bars && args.verbose {
            EnhancedUI::new(false)
        } else {
            EnhancedUI::new(true) // headless mode
        };
        ui.set_event_bus(event_bus.clone());

        // Start UI
        ui.start()?;

        if matches!(args.command, CommandKind::Code) && prompt.is_empty() {
            ui.display_error("PROMPT required for code command").await?;
            ui.finish();
            return Ok(());
        }

        let result = match args.command {
            CommandKind::Code => run_with_ui(prompt.clone(), config.clone(), event_bus.clone(), false, args.command).await,
            CommandKind::Refactor => {
                let p = if prompt.is_empty() {
                    "Analyze the current directory and perform recommended refactoring.".to_string()
                } else {
                    prompt.clone()
                };
                run_with_ui(
                    format!("Refactor codebase. {}", p),
                    config.clone(),
                    event_bus.clone(),
                    true,
                    args.command,
                )
                .await
            }
            CommandKind::Review => {
                let p = if prompt.is_empty() {
                    "ANALYSIS ONLY: Review the codebase files and create a comprehensive code review report. DO NOT generate, modify, or create any source code files. ONLY analyze existing code and document your findings, suggestions, and recommendations in code_review.md. Focus on code quality, best practices, potential issues, and improvement opportunities.".to_string()
                } else {
                    format!("ANALYSIS ONLY: Review the codebase with focus on: {}. DO NOT generate, modify, or create any source code files. ONLY analyze existing code and document your findings in code_review.md", prompt)
                };
                run_with_ui(p, config.clone(), event_bus.clone(), true, args.command).await
            }
            CommandKind::Docs => {
                let p = if prompt.is_empty() {
                    "Generate comprehensive documentation for the codebase. Create documentation files in a docs/ directory.".to_string()
                } else {
                    format!("Generate documentation for the codebase with these instructions: {}. Create documentation files in a docs/ directory.", prompt)
                };
                run_with_ui(p, config.clone(), event_bus.clone(), true, args.command).await
            }
            CommandKind::Security => {
                let p = if prompt.is_empty() {
                    "SECURITY ANALYSIS ONLY: Perform a comprehensive security analysis of the codebase. DO NOT generate, modify, or create any source code files. ONLY analyze existing code for vulnerabilities, security issues, and best practice violations. Document your findings, risk assessments, and security recommendations in security_report.md.".to_string()
                } else {
                    format!("SECURITY ANALYSIS ONLY: Perform a security analysis of the codebase focusing on: {}. DO NOT generate, modify, or create any source code files. ONLY analyze existing code and document your security findings in security_report.md", prompt)
                };
                run_with_ui(p, config.clone(), event_bus.clone(), true, args.command).await
            }
        };

        match result {
            Ok(_) => ui.finish(),
            Err(e) => {
                ui.display_error(&format!("{}", e)).await?;
                ui.finish();
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn scan_and_populate_context(
    context_manager: &ContextManager,
    context_id: &str,
    event_bus: Arc<EventBus>,
) -> Result<(usize, String)> {
    let _ = event_bus
        .emit(Event::LogLine {
            level: "INFO".to_string(),
            message: "Scanning codebase for context...".to_string(),
        })
        .await;

    let mut file_count = 0;
    let mut file_list = Vec::new();
    let current_dir = std::env::current_dir()?;
    
    // Define extensions to scan
    let code_extensions = vec![
        "rs", "py", "js", "ts", "java", "c", "cpp", "h", "hpp", "go", 
        "rb", "php", "swift", "kt", "scala", "sh", "bash", "yaml", "yml",
        "json", "toml", "xml", "html", "css", "jsx", "tsx", "vue", "svelte"
    ];
    
    let config_files = vec![
        "Cargo.toml", "package.json", "pom.xml", "build.gradle", 
        "requirements.txt", "setup.py", "Gemfile", "composer.json",
        "Makefile", "Dockerfile", ".gitignore", "README.md", "README"
    ];

    // Scan for code files
    for entry in WalkDir::new(&current_dir)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && 
            name != "target" && 
            name != "node_modules" && 
            name != "venv" &&
            name != "artifacts" &&
            name != "dist" &&
            name != "build"
        })
    {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            let file_name = path.file_name().unwrap().to_string_lossy();
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            
            // Check if it's a code file or config file
            let should_include = code_extensions.contains(&ext) || 
                                config_files.iter().any(|&cf| file_name == cf);
            
            if should_include {
                // Skip very large files
                let metadata = std::fs::metadata(&path)?;
                if metadata.len() > 100_000 {
                    info!("Skipping large file {:?} ({}KB)", path, metadata.len() / 1024);
                    continue;
                }
                
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let relative_path = path.strip_prefix(&current_dir)
                            .unwrap_or(path)
                            .to_string_lossy();
                        
                        let file_info = format!(
                            "File: {}\n```{}\n{}\n```",
                            relative_path,
                            ext.to_string(),
                            content
                        );
                        
                        context_manager
                            .add_message(context_id, "system".to_string(), file_info)
                            .await?;
                        
                        file_count += 1;
                        file_list.push(relative_path.to_string());
                        info!("Added {} to context ({} bytes)", relative_path, content.len());
                    }
                    Err(e) => {
                        warn!("Failed to read {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    event_bus
        .emit(Event::LogLine {
            level: "INFO".to_string(),
            message: format!("Scanning complete. Added {} files to context", file_count),
        })
        .await?;
    
    info!("Scan complete: added {} files to context", file_count);
    
    // Create a summary of what was scanned
    let file_summary = if file_count > 0 {
        format!("\n\nThe following {} files from this codebase have been loaded into context:\n{}", 
                file_count, 
                file_list.join("\n"))
    } else {
        String::new()
    };
    
    Ok((file_count, file_summary))
}

async fn run_with_ui(prompt: String, config: Arc<Config>, event_bus: Arc<EventBus>, scan_codebase: bool, command: CommandKind) -> Result<()> {
    let (llm_manager, artifact_manager, context_manager) =
        setup_managers(&*config, event_bus.clone()).await?;

    let task_id = Uuid::new_v4().to_string();
    event_bus
        .emit(Event::TaskStarted {
            task_id: task_id.clone(),
            description: prompt.clone(),
        })
        .await?;
    info!("Emitting TaskStarted event for task: {}", prompt);

    // Create and run agentic loop
    let agentic_loop = AgenticLoop::new(
        llm_manager.clone(),
        config.execution.max_iterations,
        event_bus.clone(),
    )
    .with_context_manager(context_manager.clone())
    .with_config(config.clone())
    .with_artifact_manager(artifact_manager.clone())
    .with_command(command);
    info!("AgenticLoop instance created.");
    let ctx_id = context_manager
        .create_context(std::collections::HashMap::new())
        .await;
    info!("Context created. Running agentic loop...");

    // Emit execution started event
    event_bus
        .emit(Event::LogLine {
            level: "INFO".to_string(),
            message: "Execution started".to_string(),
        })
        .await?;

    // Scan and populate context if requested
    let mut enhanced_prompt = prompt;
    if scan_codebase {
        let (file_count, file_summary) = scan_and_populate_context(&context_manager, &ctx_id, event_bus.clone()).await?;
        if file_count > 0 {
            // Append file summary to the prompt so the planner knows what files exist
            enhanced_prompt = format!("{}{}", enhanced_prompt, file_summary);
        }
    }

    let result = agentic_loop.run(&enhanced_prompt, &ctx_id).await;
    info!("Agentic loop completed");

    match result {
        Ok(_) => {
            info!("Task completed successfully");
            event_bus
                .emit(Event::TaskCompleted {
                    task_id: task_id.clone(),
                    result: "Success".to_string(),
                })
                .await?;
        }
        Err(ref e) => {
            error!("Task failed: {}", e);
            event_bus
                .emit(Event::TaskFailed {
                    task_id,
                    error: e.to_string(),
                })
                .await?;
        }
    }

    // Cleanup artifacts if configured
    if config.execution.cleanup_on_exit {
        info!("Cleaning up artifacts...");
        artifact_manager.cleanup().await?;
    }

    result.map(|_| ())
}

async fn setup_managers(
    config: &Config,
    event_bus: Arc<EventBus>,
) -> Result<(Arc<LLMManager>, Arc<ArtifactManager>, Arc<ContextManager>)> {
    // Initialize artifact manager
    let mut artifact_manager =
        ArtifactManager::new(std::env::current_dir()?.join(&config.execution.artifact_dir))?;
    artifact_manager.set_event_bus(event_bus.clone());
    let artifact_manager = Arc::new(artifact_manager);

    // Initialize context manager
    let context_config = ContextConfig {
        max_tokens: config.context.max_tokens,
        compression_threshold: config.context.compression_threshold,
        cache_enabled: config.context.cache_enabled,
        cache_dir: std::env::current_dir()?
            .join(".cli_engineer")
            .join("context_cache"),
    };

    let mut context_manager = ContextManager::new(context_config)?;
    context_manager.set_event_bus(event_bus.clone());

    // Initialize providers
    let mut providers: Vec<Box<dyn LLMProvider>> = Vec::new();

    if let Some(openrouter_config) = &config.ai_providers.openrouter {
        if openrouter_config.enabled {
            let provider = OpenRouterProvider::new(
                Some(openrouter_config.model.clone()),
                openrouter_config.temperature,
                openrouter_config.max_tokens,
            )?;
            providers.push(Box::new(provider));
        }
    }

    if let Some(openai_config) = &config.ai_providers.openai {
        if openai_config.enabled {
            providers.push(Box::new(OpenAIProvider::new(
                Some(openai_config.model.clone()),
                openai_config.temperature,
            )?));
        }
    }

    if let Some(anthropic_config) = &config.ai_providers.anthropic {
        if anthropic_config.enabled {
            providers.push(Box::new(AnthropicProvider::new(
                Some(anthropic_config.model.clone()),
                anthropic_config.temperature,
            )?));
        }
    }

    if providers.is_empty() {
        error!("No AI providers configured, using LocalProvider");
        providers.push(Box::new(LocalProvider));
    }

    let llm_manager = Arc::new(LLMManager::new(
        providers,
        event_bus.clone(),
        Arc::new(config.clone()),
    ));
    context_manager.set_llm_manager(llm_manager.clone());
    let context_manager = Arc::new(context_manager);

    Ok((llm_manager, artifact_manager, context_manager))
}
