use std::io::Write;
use std::time::Duration;
use std::sync::Arc;

use anyhow::Result;
use log::info;

use crate::event_bus::{EventBus, Event, EventEmitter};
use crate::impl_event_emitter;

/// Simple terminal UI handler using a spinner progress bar.
pub struct UIHandler {
    pub headless: bool,
    handle: Option<tokio::task::JoinHandle<()>>,
    event_bus: Option<Arc<EventBus>>,
}

impl UIHandler {
    pub fn new(headless: bool) -> Self { 
        Self { 
            headless, 
            handle: None,
            event_bus: None,
        } 
    }

    pub fn start(&mut self) -> Result<()> {
        if !self.headless {
            info!("Starting UI");
            
            // Clone event bus for the spawned task
            let event_bus = self.event_bus.clone();
            
            let handle = tokio::spawn(async move {
                let frames = ["|", "/", "-", "\\"];
                let mut idx = 0usize;
                
                // If we have event bus, also listen for events
                if let Some(bus) = event_bus {
                    let mut receiver = bus.subscribe();
                    
                    loop {
                        // Check for events with timeout
                        match tokio::time::timeout(
                            Duration::from_millis(100),
                            receiver.recv()
                        ).await {
                            Ok(Ok(event)) => {
                                // Handle specific events that affect UI
                                match event {
                                    Event::TaskProgress { message, .. } => {
                                        print!("\r{} {}", frames[idx % frames.len()], message);
                                    }
                                    Event::ShutdownRequested => {
                                        break;
                                    }
                                    _ => {
                                        // Continue with spinner for other events
                                        print!("\r{}", frames[idx % frames.len()]);
                                    }
                                }
                            }
                            _ => {
                                // No event or timeout, just update spinner
                                print!("\r{}", frames[idx % frames.len()]);
                            }
                        }
                        
                        let _ = std::io::stdout().flush();
                        idx = (idx + 1) % frames.len();
                    }
                } else {
                    // No event bus, just show spinner
                    loop {
                        print!("\r{}", frames[idx % frames.len()]);
                        let _ = std::io::stdout().flush();
                        idx = (idx + 1) % frames.len();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            });
            self.handle = Some(handle);
        }
        Ok(())
    }

    pub fn finish(&mut self) {
        if let Some(handle) = &self.handle {
            handle.abort();
            print!("\r \r");
            let _ = std::io::stdout().flush();
        }
        self.handle = None;
    }

    /// Display a task description
    pub async fn display_task(&mut self, task: &str) -> Result<()> {
        println!("Task: {}", task);
        Ok(())
    }
    
    /// Display an error message
    pub async fn display_error(&mut self, error: &str) -> Result<()> {
        println!("Error: {}", error);
        Ok(())
    }
}

// Implement EventEmitter trait for UIHandler
impl_event_emitter!(UIHandler);
