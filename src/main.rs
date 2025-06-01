use anyhow::Result;
use log::{info, error};
use std::sync::{Arc, Mutex};
use clap::Parser;
use uuid::Uuid;
use tokio::time::Duration;

use llm_manager::{LLMProvider, LLMManager, LocalProvider};
use agentic_loop::AgenticLoop;
use artifact::ArtifactManager;
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

#[derive(Parser, Debug)]
#[command(
    name = "cli_engineer",
    about = "A skeletal implementation of an autonomous CLI coding agent written in Rust"
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
    /// Command or natural language instruction
    #[arg(last = true)]
    command: Vec<String>,
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
    
    if args.dashboard {
        // Use dashboard UI when --dashboard is specified
        let mut ui = DashboardUI::new(false);
        ui.set_event_bus(event_bus.clone());
        
        // Start UI
        ui.start()?;
        
        // Join command into a single string
        let command = args.command.join(" ");
        
        if command.is_empty() {
            ui.display_error("No command provided")?;
            ui.finish()?;
            return Ok(());
        }
        
        // Run the main logic with dashboard UI
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
        
        match run_with_ui(command, config, event_bus.clone()).await {
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
        
        // Join command into a single string
        let command = args.command.join(" ");
        
        if command.is_empty() {
            ui.display_error("No command provided").await?;
            ui.finish();
            return Ok(());
        }
        
        // Run the main logic
        match run_with_ui(command, config, event_bus.clone()).await {
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
    // Initialize artifact manager
    let mut artifact_manager = ArtifactManager::new(std::env::current_dir()?.join(&config.execution.artifact_dir))?;
    info!("ArtifactManager initialized.");
    artifact_manager.set_event_bus(event_bus.clone());
    
    // Initialize context manager
    let context_config = ContextConfig {
        max_tokens: config.context.max_tokens,
        compression_threshold: config.context.compression_threshold,
        cache_enabled: config.context.cache_enabled,
        cache_dir: std::env::current_dir()?.join(".cli_engineer").join("context_cache"),
    };
    
    let mut context_manager = ContextManager::new(context_config)?;
    info!("ContextManager initialized.");
    context_manager.set_event_bus(event_bus.clone());
    
    // Initialize providers
    let mut providers: Vec<Box<dyn LLMProvider>> = Vec::new();
    
    // OpenRouter provider (preferred)
    if let Some(openrouter_config) = &config.ai_providers.openrouter {
        if openrouter_config.enabled {
            match OpenRouterProvider::new(
                Some(openrouter_config.model.clone()),
                openrouter_config.temperature
            ) {
                Ok(provider) => {
                    info!("Initialized OpenRouter provider");
                    providers.push(Box::new(provider));
                },
                Err(e) => error!("Failed to initialize OpenRouter provider: {}", e),
            }
        }
    }
    
    // OpenAI provider fallback
    if let Some(openai_config) = &config.ai_providers.openai {
        if openai_config.enabled {
            providers.push(Box::new(OpenAIProvider::new(
                Some(openai_config.model.clone()),
                openai_config.temperature
            )?));
            info!("Initialized OpenAI provider");
        }
    }
    
    // Anthropic provider fallback
    if let Some(anthropic_config) = &config.ai_providers.anthropic {
        if anthropic_config.enabled {
            providers.push(Box::new(AnthropicProvider::new(
                Some(anthropic_config.model.clone()),
                anthropic_config.temperature
            )?));
            info!("Initialized Anthropic provider");
        }
    }
    
    // Default to LocalProvider if no other providers are available
    if providers.is_empty() {
        error!("No AI providers configured, using LocalProvider");
        providers.push(Box::new(LocalProvider));
    }
    
    // Create LLM manager with providers
    let llm_manager = Arc::new(LLMManager::new(providers, event_bus.clone()));
    info!("LLMManager initialized.");
    
    // Share LLM manager with context manager for intelligent compression
    context_manager.set_llm_manager(llm_manager.clone());
    
    let context_manager = Arc::new(context_manager);
    
    let task_id = Uuid::new_v4().to_string();
    event_bus.emit(Event::TaskStarted { task_id: task_id.clone(), description: command.clone() }).await?;
    info!("Emitting TaskStarted event for task: {}", command);
    
    // Create and run agentic loop
    let agentic_loop = AgenticLoop::new(
        llm_manager.clone(),
        config.execution.max_iterations,
        event_bus.clone()
    )
    .with_context_manager(context_manager.clone());
    info!("AgenticLoop instance created.");
    let ctx_id = context_manager.create_context(std::collections::HashMap::new()).await;
    info!("Context created. Running agentic loop...");
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
