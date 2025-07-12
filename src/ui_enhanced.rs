use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use colored::*;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
};
use futures::executor;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::RwLock;

use crate::event_bus::{Event, EventBus, EventEmitter, Metrics};
use crate::impl_event_emitter;

/// Enhanced terminal UI with colors, progress bars, and metrics
pub struct EnhancedUI {
    headless: bool,
    multi_progress: MultiProgress,
    main_progress: Option<ProgressBar>,
    metrics_bar: Option<ProgressBar>,
    event_bus: Option<Arc<EventBus>>,
    start_time: Instant,
    last_metrics: Arc<RwLock<Metrics>>,
}

impl EnhancedUI {
    pub fn new(headless: bool) -> Self {
        Self {
            headless,
            multi_progress: MultiProgress::new(),
            main_progress: None,
            metrics_bar: None,
            event_bus: None,
            start_time: Instant::now(),
            last_metrics: Arc::new(RwLock::new(Metrics::default())),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.headless {
            return Ok(());
        }

        // Clear screen and print header
        execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        println!("{}", "=".repeat(80).bright_blue());
        println!(
            "{}",
            "cli_engineer - Autonomous Coding Agent"
                .bright_white()
                .bold()
        );
        println!("{}", "=".repeat(80).bright_blue());
        println!();

        // Create main progress bar
        let main_progress = self.multi_progress.add(ProgressBar::new(100));
        main_progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] {wide_bar:.cyan/blue} {pos}% {msg}")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        main_progress.set_message("Initializing...");
        self.main_progress = Some(main_progress);

        // Create metrics bar
        let metrics_bar = self.multi_progress.add(ProgressBar::new(0));
        metrics_bar.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
        self.metrics_bar = Some(metrics_bar);

        // Start event handler
        if let Some(bus) = &self.event_bus {
            let multi_progress = self.multi_progress.clone();
            let main_progress = self.main_progress.clone();
            let metrics_bar = self.metrics_bar.clone();
            let last_metrics = self.last_metrics.clone();
            let mut receiver = bus.subscribe();

            tokio::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(event) => {
                            Self::handle_event(
                                event,
                                &multi_progress,
                                &main_progress,
                                &metrics_bar,
                                &last_metrics,
                            )
                            .await;
                        }
                        Err(_) => break,
                    }
                }
            });

            // Start metrics updater
            let bus = bus.clone();
            let metrics_bar = self.metrics_bar.clone();
            let last_metrics = self.last_metrics.clone();
            let start_time = self.start_time;

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    let metrics = bus.get_metrics().await;
                    *last_metrics.write().await = metrics.clone();

                    if let Some(bar) = &metrics_bar {
                        let elapsed = start_time.elapsed().as_secs();
                        let status = format!(
                            "{} | {} | {} | {} | {} | {}",
                            format!("⏱️  {:02}:{:02}", elapsed / 60, elapsed % 60).bright_white(),
                            format!(
                                "📊 Tasks: {}/{}",
                                metrics.tasks_completed,
                                metrics.tasks_completed + metrics.tasks_failed
                            )
                            .bright_green(),
                            format!("🤖 API Calls: {}", metrics.total_api_calls).bright_cyan(),
                            format!("💰 Cost: ${:.4}", metrics.total_cost).bright_yellow(),
                            format!("📝 Artifacts: {}", metrics.artifacts_created).bright_magenta(),
                            format!("💾 Context: {:.0}%", metrics.current_context_usage)
                                .bright_blue(),
                        );
                        bar.set_message(status);
                    }
                }
            });
        }

        Ok(())
    }

    pub fn finish(&mut self) {
        if self.headless {
            return;
        }

        // Show final summary
        let metrics = executor::block_on(async { self.last_metrics.read().await.clone() });

        println!();
        println!("{}", "=".repeat(80).bright_blue());
        println!("{}", "Session Summary".bright_white().bold());
        println!("{}", "=".repeat(80).bright_blue());

        let elapsed = self.start_time.elapsed();
        println!(
            "⏱️  Duration: {}:{:02}",
            elapsed.as_secs() / 60,
            elapsed.as_secs() % 60
        );
        println!(
            "✅ Tasks Completed: {}",
            metrics.tasks_completed.to_string().bright_green()
        );
        println!(
            "❌ Tasks Failed: {}",
            metrics.tasks_failed.to_string().bright_red()
        );
        println!(
            "🤖 Total API Calls: {}",
            metrics.total_api_calls.to_string().bright_cyan()
        );
        println!(
            "🪙  Total Tokens: {}",
            metrics.total_tokens.to_string().bright_cyan()
        );
        println!(
            "💰 Total Cost: ${:.4}",
            metrics.total_cost.to_string().bright_yellow()
        );
        println!(
            "📝 Artifacts Created: {}",
            metrics.artifacts_created.to_string().bright_magenta()
        );
        println!();

        if let Some(pb) = &self.main_progress {
            pb.finish_with_message("Done!");
        }
    }

    #[allow(dead_code)]
    pub async fn display_message(&mut self, message: &str) -> Result<()> {
        println!("{}", message);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn display_task(&mut self, task: &str) -> Result<()> {
        println!("{} {}", "▶ Task:".cyan().bold(), task.white());
        Ok(())
    }

    pub async fn display_error(&mut self, error: &str) -> Result<()> {
        println!("{} {}", "✗ Error:".red().bold(), error.white());
        Ok(())
    }

    async fn handle_event(
        event: Event,
        _multi_progress: &MultiProgress,
        main_progress: &Option<ProgressBar>,
        _metrics_bar: &Option<ProgressBar>,
        _last_metrics: &Arc<RwLock<Metrics>>,
    ) {
        match event {
            Event::TaskStarted { description, .. } => {
                if let Some(pb) = main_progress {
                    pb.set_message(format!("🚀 {}", description));
                    pb.set_position(0);
                }
            }
            Event::TaskProgress {
                progress, message, ..
            } => {
                if let Some(pb) = main_progress {
                    pb.set_position(progress as u64);
                    pb.set_message(format!("⚡ {}", message));
                }
            }
            Event::TaskCompleted { result, .. } => {
                if let Some(pb) = main_progress {
                    pb.set_position(100);
                    pb.set_message(format!("✅ {}", result));
                }
            }
            Event::TaskFailed { error, .. } => {
                if let Some(pb) = main_progress {
                    pb.set_message(format!("❌ {}", error.bright_red()));
                }
            }
            Event::ExecutionStarted { environment } => {
                if let Some(pb) = main_progress {
                    pb.set_message(format!("🔧 Executing in {}", environment));
                }
            }
            Event::ExecutionProgress { step, progress } => {
                if let Some(pb) = main_progress {
                    pb.set_position(progress as u64);
                    pb.set_message(format!("🔨 {}", step));
                }
            }
            Event::DependencyInstalling { package } => {
                if let Some(pb) = main_progress {
                    pb.set_message(format!("📦 Installing {}", package.bright_cyan()));
                }
            }
            Event::ArtifactCreated {
                name,
                artifact_type,
                ..
            } => {
                if let Some(pb) = main_progress {
                    pb.set_message(format!(
                        "📄 Created {} ({})",
                        name.bright_green(),
                        artifact_type
                    ));
                }
            }
            Event::APICallStarted { provider, model } => {
                if let Some(pb) = main_progress {
                    pb.set_message(format!("🤖 Calling {} ({})", provider.bright_cyan(), model));
                }
            }
            _ => {}
        }
    }
}

// Implement EventEmitter trait for EnhancedUI
impl_event_emitter!(EnhancedUI);
