use log::{LevelFilter, Metadata, Record, SetLoggerError};
use std::sync::{Arc, Mutex};
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Utc;
use tokio;

use crate::event_bus::{Event, EventBus};

pub struct DashboardLogger {
    pub event_bus: Arc<EventBus>,
    pub level: LevelFilter,
    pub file_writer: Option<Arc<Mutex<std::fs::File>>>,
}

impl log::Log for DashboardLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level.to_level().unwrap_or(log::Level::Error)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let timestamp = Utc::now().format("%H:%M:%S");
            let msg = format!("{}", record.args());

            // Write to file if file writer is available
            if let Some(file_writer) = &self.file_writer {
                if let Ok(mut file) = file_writer.lock() {
                    let log_line = format!("{} [{}] {}\n", timestamp, record.level(), msg);
                    let _ = file.write_all(log_line.as_bytes());
                    let _ = file.flush();
                }
            }

            // Emit to dashboard
            let event_bus = self.event_bus.clone();
            let level = record.level().to_string();
            let message = msg.clone();

            tokio::spawn(async move {
                let _ = event_bus.emit(Event::LogLine { level, message }).await;
            });
        }
    }

    fn flush(&self) {
        if let Some(file_writer) = &self.file_writer {
            if let Ok(mut file) = file_writer.lock() {
                let _ = file.flush();
            }
        }
    }
}

impl DashboardLogger {
    pub fn init_with_file(event_bus: Arc<EventBus>, level: LevelFilter, enable_file_logging: bool) -> Result<(), SetLoggerError> {
        let file_writer = if enable_file_logging {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let log_filename = format!("cli_engineer_{}.log", timestamp);
            
            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_filename) 
            {
                Ok(mut file) => {
                    let session_start = format!("=== CLI Engineer Session Started: {} UTC ===\n\n", 
                                               Utc::now().format("%Y-%m-%d %H:%M:%S"));
                    let _ = file.write_all(session_start.as_bytes());
                    let _ = file.flush();
                    
                    log::info!("Verbose logging enabled. Session details will be logged to: {}", log_filename);
                    Some(Arc::new(Mutex::new(file)))
                }
                Err(e) => {
                    eprintln!("Warning: Could not create log file: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let logger = Box::leak(Box::new(DashboardLogger { 
            event_bus, 
            level,
            file_writer,
        }));
        log::set_logger(logger)?;
        log::set_max_level(level);
        Ok(())
    }
}
