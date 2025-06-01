use std::time::{Duration, Instant};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use crossterm::{
    terminal::{size, Clear, ClearType},
    cursor::{Hide, Show, MoveTo},
    execute
};
use colored::*;
use anyhow::Result;
use crate::event_bus::{EventBus, Event, EventEmitter};
use crate::impl_event_emitter;
use tokio;

/// Dashboard UI that updates in-place without scrolling
use std::collections::VecDeque;

pub struct DashboardUI {
    headless: bool,
    event_bus: Option<Arc<EventBus>>,
    start_time: Instant,
    // Log buffer
    log_lines: Arc<Mutex<VecDeque<String>>>,
    
    // Current status
    current_phase: Arc<Mutex<String>>,
    current_task: Arc<Mutex<String>>,
    current_status: Arc<Mutex<String>>,
    progress: Arc<Mutex<f32>>,
    
    // Metrics
    api_calls: Arc<Mutex<usize>>,
    artifacts_created: Arc<Mutex<usize>>,
    tasks_completed: Arc<Mutex<usize>>,
    tasks_total: Arc<Mutex<usize>>,
    total_cost: Arc<Mutex<f64>>,
    context_usage: Arc<Mutex<f32>>,
    last_update: Instant,
}

impl DashboardUI {
    pub fn new(headless: bool) -> Self {
        Self {
            headless,
            event_bus: None,
            start_time: Instant::now(),
            current_phase: Arc::new(Mutex::new("Initializing".to_string())),
            current_task: Arc::new(Mutex::new(String::new())),
            current_status: Arc::new(Mutex::new(String::new())),
            progress: Arc::new(Mutex::new(0.0)),
            api_calls: Arc::new(Mutex::new(0)),
            artifacts_created: Arc::new(Mutex::new(0)),
            tasks_completed: Arc::new(Mutex::new(0)),
            tasks_total: Arc::new(Mutex::new(0)),
            total_cost: Arc::new(Mutex::new(0.0)),
            context_usage: Arc::new(Mutex::new(0.0)),
            last_update: Instant::now(),
            log_lines: Arc::new(Mutex::new(VecDeque::with_capacity(30))),
        }
    }
    
    pub fn start(&mut self) -> Result<()> {
        if self.headless {
            return Ok(());
        }
        
        // Hide cursor and clear screen
        execute!(io::stdout(), Hide, Clear(ClearType::All))?;
        
        // Initial render
        self.render()?;
        
        // Start background event listener if event bus is available
        if let Some(event_bus) = &self.event_bus {
            let receiver = event_bus.subscribe();
            let log_lines = self.log_lines.clone();
            let current_phase = self.current_phase.clone();
            let current_task = self.current_task.clone();
            let current_status = self.current_status.clone();
            let progress = self.progress.clone();
            let api_calls = self.api_calls.clone();
            let artifacts_created = self.artifacts_created.clone();
            let tasks_completed = self.tasks_completed.clone();
            let tasks_total = self.tasks_total.clone();
            let total_cost = self.total_cost.clone();
            let context_usage = self.context_usage.clone();
            
            tokio::spawn(async move {
                let mut event_receiver = receiver;
                while let Ok(event) = event_receiver.recv().await {
                    match event {
                        Event::LogLine { level, message } => {
                            let colored = match level.as_str() {
                                "ERROR" => format!("[ERROR] {}", message).red().to_string(),
                                "WARN" => format!("[WARN ] {}", message).yellow().to_string(),
                                "INFO" => format!("[INFO ] {}", message).cyan().to_string(),
                                "DEBUG" => format!("[DEBUG] {}", message).white().to_string(),
                                "TRACE" => format!("[TRACE] {}", message).dimmed().to_string(),
                                _ => format!("[{}] {}", level, message),
                            };
                            let mut logs = log_lines.lock().unwrap();
                            if logs.len() >= 30 { logs.pop_front(); }
                            logs.push_back(colored.clone());
                        }
                        Event::TaskStarted { description, .. } => {
                            *current_task.lock().unwrap() = description;
                            *current_status.lock().unwrap() = "Running".to_string();
                        }
                        Event::TaskCompleted { .. } => {
                            *current_status.lock().unwrap() = "Completed".to_string();
                            *progress.lock().unwrap() = 1.0;
                            *tasks_completed.lock().unwrap() += 1;
                        }
                        Event::ExecutionStarted { .. } => {
                            *tasks_total.lock().unwrap() += 1;
                            let iter_count = *tasks_total.lock().unwrap();
                            *current_phase.lock().unwrap() = format!("Iteration {}", iter_count);
                        }
                        Event::APICallStarted { provider, model } => {
                            *api_calls.lock().unwrap() += 1;
                            *current_status.lock().unwrap() = format!("Calling {}/{}", provider, model);
                        }
                        Event::APICallCompleted { cost, .. } => {
                            *total_cost.lock().unwrap() += cost as f64;
                            *current_status.lock().unwrap() = "API response received".to_string();
                        }
                        Event::ArtifactCreated { .. } => {
                            *artifacts_created.lock().unwrap() += 1;
                        }
                        Event::ContextUsageChanged { usage_percentage, .. } => {
                            *context_usage.lock().unwrap() = usage_percentage;
                        }
                        _ => {}
                    }
                }
            });
        }
        
        Ok(())
    }
    
    pub fn finish(&mut self) -> Result<()> {
        if self.headless {
            return Ok(());
        }
        
        // Show cursor again
        execute!(io::stdout(), Show)?;
        
        // Move to bottom and print summary
        let (_, height) = size()?;
        execute!(io::stdout(), MoveTo(0, height - 2))?;
        
        let elapsed = self.start_time.elapsed();
        println!("\n{}", "=".repeat(120).bright_blue());
        println!("{} {} in {:.1}s", 
            "✓".green().bold(),
            "Task completed".bright_white().bold(),
            elapsed.as_secs_f32()
        );
        println!("  {} iterations | {} API calls | {} artifacts | ${:.3} cost",
            self.tasks_total.lock().unwrap().to_string().cyan(),
            self.api_calls.lock().unwrap().to_string().yellow(),
            self.artifacts_created.lock().unwrap().to_string().green(),
            format!("{:.3}", self.total_cost.lock().unwrap()).magenta()
        );
        
        Ok(())
    }
    
    fn render(&self) -> Result<()> {
        if self.headless {
            return Ok(());
        }
        
        // Calculate elapsed time
        let elapsed = self.start_time.elapsed();
        let minutes = elapsed.as_secs() / 60;
        let seconds = elapsed.as_secs() % 60;
        
        // Clear and move to top
        execute!(io::stdout(), MoveTo(0, 0))?;
        
        // Box width constants
        const _BOX_WIDTH: usize = 120;
        const CONTENT_WIDTH: usize = 118; // BOX_WIDTH - 2 (for borders)
        
        // Header
        println!("{}", "╔══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╗".bright_blue());
        
        // Title line with time
        let title = "CLI Engineer";
        let time_str = format!("{}:{:02}", minutes, seconds);
        let padding = CONTENT_WIDTH.saturating_sub(title.len() + time_str.len() + 3);
        println!("{} {}{}{} {}{}", 
            "║".bright_blue(),
            title.bright_white().bold(),
            " ".repeat(padding),
            time_str,
            " ", // add 1 space after time
            "║".bright_blue()
        );
        
        println!("{}", "╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╣".bright_blue());
        
        // Phase and Progress
        let phase_label = "Phase: ";
        let phase_text = if let Ok(guard) = self.current_phase.try_lock() {
            guard.clone()
        } else {
            "Loading...".to_string()
        };
        let progress_bar_str = self.render_progress_bar(60);
        let progress_bar_visible = strip_ansi_codes(&progress_bar_str);
        let progress_bar_width = progress_bar_visible.len();
        
        // Calculate padding: distribute space before and after progress bar
        let used_width = phase_label.len() + phase_text.len() + 1 + progress_bar_width; // 1 space after phase_text
        let total_padding = CONTENT_WIDTH.saturating_sub(used_width);
        let right_padding = 32; // Fixed right padding to ensure proper alignment
        let left_padding = total_padding.saturating_sub(right_padding);
        
        print!("{}{}{}", "║".bright_blue(), phase_label.bright_white(), phase_text.cyan());
        print!(" {}", " ".repeat(left_padding));
        print!("{}", progress_bar_str);
        print!("{}", " ".repeat(right_padding));
        println!("{}", " ║".bright_blue());
        io::stdout().flush()?;
        
        // Current Task
        let task_label = "Task: ";
        let max_task_len = CONTENT_WIDTH.saturating_sub(task_label.len() + 1);
        let task_text = if let Ok(guard) = self.current_task.try_lock() {
            if guard.len() > max_task_len {
                let end_idx = guard.len().min(max_task_len.saturating_sub(3));
                format!("{}...", &guard[..end_idx])
            } else {
                guard.clone()
            }
        } else {
            "Loading...".to_string()
        };
        let task_padding = CONTENT_WIDTH.saturating_sub(task_label.len() + task_text.len() + 1);
        
        print!("{} {}{}", "║".bright_blue(), task_label.bright_white(), task_text.yellow());
        print!("{}", " ".repeat(task_padding));
        println!("{}", "║".bright_blue());
        io::stdout().flush()?;
        
        // Status
        let status_label = "Status: ";
        let max_status_len = CONTENT_WIDTH.saturating_sub(status_label.len() + 1);
        let status_text = if let Ok(guard) = self.current_status.try_lock() {
            if guard.len() > max_status_len {
                let end_idx = guard.len().min(max_status_len.saturating_sub(3));
                format!("{}...", &guard[..end_idx])
            } else {
                guard.clone()
            }
        } else {
            "Loading...".to_string()
        };
        let status_color = if status_text.starts_with("✅") {
            status_text.green()
        } else if status_text.starts_with("❌") {
            status_text.red()
        } else {
            status_text.white()
        };
        let status_padding = CONTENT_WIDTH.saturating_sub(status_label.len() + status_text.len() + 1);
        
        print!("{} {}{}", "║".bright_blue(), status_label.bright_white(), status_color);
        print!("{}", " ".repeat(status_padding));
        println!("{}", "║".bright_blue());
        io::stdout().flush()?;
        
        println!("{}", "╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╣".bright_blue());
        
        // Metrics - build the complete metrics line first
        let api_calls = if let Ok(guard) = self.api_calls.try_lock() { *guard } else { 0 };
        let artifacts = if let Ok(guard) = self.artifacts_created.try_lock() { *guard } else { 0 };
        let tasks_completed = if let Ok(guard) = self.tasks_completed.try_lock() { *guard } else { 0 };
        let tasks_total = if let Ok(guard) = self.tasks_total.try_lock() { *guard } else { 0 };
        let total_cost = if let Ok(guard) = self.total_cost.try_lock() { *guard } else { 0.0 };
        let context_usage = if let Ok(guard) = self.context_usage.try_lock() { *guard } else { 0.0 };
        
        let formatted_cost = format!("{:.3}", total_cost);
        let formatted_tasks = format!("{}/{}", tasks_completed, tasks_total);
        let formatted_api_calls = api_calls.to_string();
        let formatted_artifacts = artifacts.to_string();
        let formatted_context = format!("{:.1}", context_usage);
        
        // Calculate the actual content length that will be printed - EXACTLY as printed
        let actual_content = format!("📊 Tasks: {} | 🤖 API Calls: {} | 💰 Cost: ${} | 📝 Artifacts: {} | 💾 Context: {}%", 
            formatted_tasks, formatted_api_calls, formatted_cost, formatted_artifacts, formatted_context);
        // Account for emoji display width (emojis take more visual space than character count)
        let emoji_display_adjustment = 10; // Based on debug output showing 10 missing spaces
        let metrics_padding = CONTENT_WIDTH.saturating_sub(actual_content.len() + 1 - emoji_display_adjustment);
        
        print!("{} ", "║".bright_blue());
        print!("📊 Tasks: {} | 🤖 API Calls: {} | 💰 Cost: ${} | 📝 Artifacts: {} | 💾 Context: {}%", 
            formatted_tasks.cyan(),
            formatted_api_calls.yellow(),
            formatted_cost.green(),
            formatted_artifacts.green(), 
            formatted_context);
        print!("{}", " ".repeat(metrics_padding));
        println!("{}", "║".bright_blue());
        println!("{}", "╠══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╣".bright_blue());
        io::stdout().flush()?;
        
        // Log area
        let log_lines = if let Ok(guard) = self.log_lines.try_lock() {
            guard.clone()
        } else {
            std::collections::VecDeque::new()
        };
        
        for log_line in log_lines.iter() {
            let max_log_len = CONTENT_WIDTH.saturating_sub(1); // Leave 1 space for right border
            let visible_log = strip_ansi_codes(log_line);
            let truncated_log = if visible_log.len() > max_log_len {
                // Use char_indices to find safe character boundaries
                let truncate_at = visible_log.char_indices()
                    .nth(max_log_len.saturating_sub(3))
                    .map(|(i, _)| i)
                    .unwrap_or(visible_log.len());
                format!("{}...", &visible_log[..truncate_at])
            } else {
                log_line.clone()
            };
            let visible_truncated = strip_ansi_codes(&truncated_log);
            let log_padding = CONTENT_WIDTH.saturating_sub(visible_truncated.len() + 1); // +1 for the space after ║
            print!("{} {}{}", "║".bright_blue(), truncated_log, " ".repeat(log_padding));
            println!("{}", "║".bright_blue());
            io::stdout().flush()?;
        }
        
        // Add blank lines to fill up the log area if needed
        let total_log_lines: usize = 30;
        let blank_lines = total_log_lines.saturating_sub(log_lines.len());
        for _ in 0..blank_lines {
            let log_padding = CONTENT_WIDTH - 1; // Leave 1 space for the content, not 2
            print!("{} {}", "║".bright_blue(), " ".repeat(log_padding));
            println!("{}", "║".bright_blue());
            io::stdout().flush()?;
        }
        
        println!("{}", "╚══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════╝".bright_blue());
        
        // Flush output
        io::stdout().flush()?;
        
        Ok(())
    }
    
    fn render_progress_bar(&self, width: usize) -> String {
        let progress_val = if let Ok(guard) = self.progress.try_lock() { 
            *guard 
        } else { 
            0.0 
        };
        let filled = ((progress_val * width as f32) as usize).min(width);
        let empty = width - filled;
        
        format!("[{}{}] {:.0}%",
            "█".repeat(filled).green(),
            "─".repeat(empty).bright_black(),
            progress_val * 100.0
        )
    }
    
    #[allow(dead_code)]
    pub fn update_phase(&mut self, phase: &str) -> Result<()> {
        *self.current_phase.lock().unwrap() = phase.to_string();
        *self.progress.lock().unwrap() = 0.0;
        self.throttled_render()
    }
    
    #[allow(dead_code)]
    pub fn update_task(&mut self, task: &str) -> Result<()> {
        *self.current_task.lock().unwrap() = task.to_string();
        self.throttled_render()
    }
    
    pub fn update_status(&mut self, status: &str) -> Result<()> {
        *self.current_status.lock().unwrap() = status.to_string();
        self.throttled_render()
    }
    
    #[allow(dead_code)]
    pub fn update_progress(&mut self, progress: f32) -> Result<()> {
        *self.progress.lock().unwrap() = progress.clamp(0.0, 1.0);
        self.throttled_render()
    }
    
    #[allow(dead_code)]
    pub fn update_metrics(&mut self, api_calls: usize, artifacts: usize, tasks_completed: usize, tasks_total: usize, total_cost: f64) -> Result<()> {
        *self.api_calls.lock().unwrap() = api_calls;
        *self.artifacts_created.lock().unwrap() = artifacts;
        *self.tasks_completed.lock().unwrap() = tasks_completed;
        *self.tasks_total.lock().unwrap() = tasks_total;
        *self.total_cost.lock().unwrap() = total_cost;
        self.throttled_render()
    }
    
    /// Only render if enough time has passed to avoid flickering
    pub fn throttled_render(&mut self) -> Result<()> {
        if self.last_update.elapsed() > Duration::from_millis(100) {
            self.last_update = Instant::now();
            self.render()?;
        }
        Ok(())
    }
    
    #[allow(dead_code)]
    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::LogLine { level, message } => {
                let colored = match level.as_str() {
                    "ERROR" => format!("[ERROR] {}", message).red().to_string(),
                    "WARN" => format!("[WARN ] {}", message).yellow().to_string(),
                    "INFO" => format!("[INFO ] {}", message).cyan().to_string(),
                    "DEBUG" => format!("[DEBUG] {}", message).white().to_string(),
                    "TRACE" => format!("[TRACE] {}", message).dimmed().to_string(),
                    _ => format!("[{}] {}", level, message),
                };
                let mut logs = self.log_lines.lock().unwrap();
                if logs.len() >= 30 { logs.pop_front(); }
                logs.push_back(colored.clone());
            }
            Event::TaskStarted { description, .. } => {
                self.update_task(&description)?;
                self.update_status("Running")?;
            }
            Event::TaskCompleted { .. } => {
                self.update_status("Completed")?;
                self.update_progress(1.0)?;
                *self.tasks_completed.lock().unwrap() += 1;
            }
            Event::ExecutionStarted { .. } => {
                *self.tasks_total.lock().unwrap() += 1;
                let iter_count = *self.tasks_total.lock().unwrap();
                self.update_phase(&format!("Iteration {}", iter_count))?;
            }
            Event::APICallStarted { provider, model } => {
                *self.api_calls.lock().unwrap() += 1;
                self.update_status(&format!("Calling {}/{}", provider, model))?;
            }
            Event::APICallCompleted { cost, .. } => {
                *self.total_cost.lock().unwrap() += cost as f64;
                self.update_status("API response received")?;
            }
            Event::ArtifactCreated { .. } => {
                *self.artifacts_created.lock().unwrap() += 1;
            }
            Event::ContextUsageChanged { usage_percentage, .. } => {
                *self.context_usage.lock().unwrap() = usage_percentage;
            }
            _ => {}
        }
        Ok(())
    }
    
    pub fn display_error(&mut self, error: &str) -> Result<()> {
        self.update_status(&format!("❌ {}", error))
    }
    
    #[allow(dead_code)]
    pub fn display_task(&mut self, task: &str) -> Result<()> {
        self.update_task(task)
    }
}

// Implement EventEmitter trait
impl_event_emitter!(DashboardUI);

// Helper to strip ANSI escape codes
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip until 'm'
            while let Some(nc) = chars.next() {
                if nc == 'm' { break; }
            }
        } else {
            result.push(c);
        }
    }
    result
}
