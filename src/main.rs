use anyhow::Result;
use log::{info, error};
use std::sync::{Arc, Mutex};
use clap::{Parser, ValueEnum};
use uuid::Uuid;
use tokio::time::Duration;

use llm_manager::{LLMProvider, LLMManager, LocalProvider};
use agentic_loop::AgenticLoop;
use artifact::{ArtifactManager, ArtifactType};
use config::Config;
use event_bus::{EventBus, Event, EventEmitter};
use context::{ContextManager, ContextConfig};
use providers::{
    openai::OpenAIProvider,
    anthropic::AnthropicProvider,
    openrouter::OpenRouterProvider,
};
use ui_enhanced::EnhancedUI;
use ui_dashboard::DashboardUI;
mod logger_dashboard;

mod llm_manager;
mod interpreter;
mod planner;
mod executor;
mod reviewer;
mod agentic_loop;
mod concurrency;
mod ui_enhanced;
mod ui_dashboard;
mod logger;
mod config;
mod event_bus;
mod providers;
mod artifact;
mod context;
mod iteration_context;

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
        let level = if args.verbose { log::LevelFilter::Info } else { log::LevelFilter::Warn };
        logger_dashboard::DashboardLogger::init(event_bus.clone(), level).expect("Failed to init DashboardLogger");
    } else {
        logger::init(args.verbose);
    }
    
    // Load configuration
    let config = Config::load(&args.config)?;
    
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
        
        // Start periodic UI updates
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if let Ok(mut ui_guard) = ui_clone.try_lock() {
                    let _ = ui_guard.throttled_render();
                }
            }
        });
        
        let result = match args.command {
            CommandKind::Code => run_with_ui(prompt.clone(), config, event_bus.clone()).await,
            CommandKind::Refactor => {
                let p = if prompt.is_empty() {
                    "Analyze the current directory and perform recommended refactoring.".to_string()
                } else {
                    prompt.clone()
                };
                run_with_ui(format!("Refactor codebase. {}", p), config, event_bus.clone()).await
            }
            CommandKind::Review => run_review(prompt.clone(), config, event_bus.clone()).await,
            CommandKind::Docs => run_docs(prompt.clone(), config, event_bus.clone()).await,
            CommandKind::Security => run_security(prompt.clone(), config, event_bus.clone()).await,
        };

        match result {
            Ok(_) => {
                if let Ok(mut ui_guard) = ui_ref.try_lock() {
                    ui_guard.finish()?;
                }
            }
            Err(e) => {
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
            CommandKind::Code => run_with_ui(prompt.clone(), config, event_bus.clone()).await,
            CommandKind::Refactor => {
                let p = if prompt.is_empty() {
                    "Analyze the current directory and perform recommended refactoring.".to_string()
                } else {
                    prompt.clone()
                };
                run_with_ui(format!("Refactor codebase. {}", p), config, event_bus.clone()).await
            }
            CommandKind::Review => run_review(prompt.clone(), config, event_bus.clone()).await,
            CommandKind::Docs => run_docs(prompt.clone(), config, event_bus.clone()).await,
            CommandKind::Security => run_security(prompt.clone(), config, event_bus.clone()).await,
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

async fn run_with_ui(command: String, config: Config, event_bus: Arc<EventBus>) -> Result<()> {
    let (llm_manager, artifact_manager, context_manager) =
        setup_managers(&config, event_bus.clone()).await?;
    
    let task_id = Uuid::new_v4().to_string();
    event_bus.emit(Event::TaskStarted { task_id: task_id.clone(), description: command.clone() }).await?;
    info!("Emitting TaskStarted event for task: {}", command);
    
    // Create and run agentic loop
    let agentic_loop = AgenticLoop::new(
        llm_manager.clone(),
        config.execution.max_iterations,
        event_bus.clone()
    )
    .with_context_manager(context_manager.clone())
    .with_config(Arc::new(config.clone()))
    .with_artifact_manager(artifact_manager.clone());
    info!("AgenticLoop instance created.");
    let ctx_id = context_manager.create_context(std::collections::HashMap::new()).await;
    info!("Context created. Running agentic loop...");
    
    // Emit execution started event
    event_bus.emit(Event::ExecutionStarted { 
        environment: "agentic_loop".to_string() 
    }).await?;
    
    let result = agentic_loop.run(&command, &ctx_id).await;
    info!("Agentic loop completed");
    
    match result {
        Ok(_) => {
            info!("Task completed successfully");
            event_bus.emit(Event::TaskCompleted { 
                task_id: task_id.clone(), 
                result: "Success".to_string() 
            }).await?;
        }
        Err(ref e) => {
            error!("Task failed: {}", e);
            event_bus.emit(Event::TaskFailed { 
                task_id,
                error: e.to_string()
            }).await?;
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
        cache_dir: std::env::current_dir()?.join(".cli_engineer").join("context_cache"),
    };

    let mut context_manager = ContextManager::new(context_config)?;
    context_manager.set_event_bus(event_bus.clone());

    // Initialize providers
    let mut providers: Vec<Box<dyn LLMProvider>> = Vec::new();

    if let Some(openrouter_config) = &config.ai_providers.openrouter {
        if openrouter_config.enabled {
            match OpenRouterProvider::new(
                Some(openrouter_config.model.clone()),
                openrouter_config.temperature,
            ) {
                Ok(p) => {
                    providers.push(Box::new(p));
                }
                Err(e) => error!("Failed to initialize OpenRouter provider: {}", e),
            }
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

    let llm_manager = Arc::new(LLMManager::new(providers, event_bus.clone(), Arc::new(config.clone())));
    context_manager.set_llm_manager(llm_manager.clone());
    let context_manager = Arc::new(context_manager);

    Ok((llm_manager, artifact_manager, context_manager))
}

async fn run_review(prompt: String, config: Config, event_bus: Arc<EventBus>) -> Result<()> {
    let (llm_manager, artifact_manager, context_manager) =
        setup_managers(&config, event_bus.clone()).await?;

    let ctx_id = context_manager.create_context(std::collections::HashMap::new()).await;

    let base_prompt = "You are a senior engineer performing a comprehensive code review of the current project. Provide actionable recommendations.";
    let final_prompt = if prompt.is_empty() {
        base_prompt.to_string()
    } else {
        format!("{}\n\nAdditional instructions: {}", base_prompt, prompt)
    };

    event_bus
        .emit(Event::ExecutionStarted {
            environment: "review".to_string(),
        })
        .await?;

    let review_text = llm_manager.send_prompt(&final_prompt).await?;

    artifact_manager
        .create_artifact(
            "code_review".to_string(),
            ArtifactType::Documentation,
            review_text.clone(),
            std::collections::HashMap::new(),
        )
        .await?;

    println!("{}", review_text);

    event_bus
        .emit(Event::TaskCompleted {
            task_id: "review".to_string(),
            result: "Success".to_string(),
        })
        .await?;

    if config.execution.cleanup_on_exit {
        artifact_manager.cleanup().await?;
    }

    Ok(())
}

async fn run_docs(prompt: String, config: Config, event_bus: Arc<EventBus>) -> Result<()> {
    let (llm_manager, artifact_manager, context_manager) =
        setup_managers(&config, event_bus.clone()).await?;

    let ctx_id = context_manager.create_context(std::collections::HashMap::new()).await;

    let base_prompt = "Generate comprehensive documentation for the current codebase. Place outputs in the docs/ directory.";
    let final_prompt = if prompt.is_empty() {
        base_prompt.to_string()
    } else {
        format!("{}\n\nAdditional instructions: {}", base_prompt, prompt)
    };

    event_bus
        .emit(Event::ExecutionStarted {
            environment: "docs".to_string(),
        })
        .await?;

    let doc_text = llm_manager.send_prompt(&final_prompt).await?;

    artifact_manager
        .create_artifact(
            "docs/overview".to_string(),
            ArtifactType::Documentation,
            doc_text.clone(),
            std::collections::HashMap::new(),
        )
        .await?;

    println!("{}", doc_text);

    event_bus
        .emit(Event::TaskCompleted {
            task_id: "docs".to_string(),
            result: "Success".to_string(),
        })
        .await?;

    if config.execution.cleanup_on_exit {
        artifact_manager.cleanup().await?;
    }

    Ok(())
}

async fn run_security(prompt: String, config: Config, event_bus: Arc<EventBus>) -> Result<()> {
    let (llm_manager, artifact_manager, context_manager) =
        setup_managers(&config, event_bus.clone()).await?;

    let ctx_id = context_manager.create_context(std::collections::HashMap::new()).await;

    let base_prompt = "Perform a security-focused review of the current project. Identify vulnerabilities and areas of concern.";
    let final_prompt = if prompt.is_empty() {
        base_prompt.to_string()
    } else {
        format!("{}\n\nAdditional instructions: {}", base_prompt, prompt)
    };

    event_bus
        .emit(Event::ExecutionStarted {
            environment: "security".to_string(),
        })
        .await?;

    let sec_text = llm_manager.send_prompt(&final_prompt).await?;

    artifact_manager
        .create_artifact(
            "security".to_string(),
            ArtifactType::Documentation,
            sec_text.clone(),
            std::collections::HashMap::new(),
        )
        .await?;

    println!("{}", sec_text);

    event_bus
        .emit(Event::TaskCompleted {
            task_id: "security".to_string(),
            result: "Success".to_string(),
        })
        .await?;

    if config.execution.cleanup_on_exit {
        artifact_manager.cleanup().await?;
    }

    Ok(())
}
