use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use log::info;

/// Simple terminal UI handler using a spinner progress bar.
pub struct UIHandler {
    pub headless: bool,
    handle: Option<tokio::task::JoinHandle<()>>, 
}

impl UIHandler {
    pub fn new(headless: bool) -> Self { Self { headless, handle: None } }

    pub fn start(&mut self) -> Result<()> {
        if !self.headless {
            info!("Starting UI");
            let handle = tokio::spawn(async {
                let frames = ["|", "/", "-", "\\"];
                let mut idx = 0usize;
                loop {
                    print!("\r{}", frames[idx % frames.len()]);
                    let _ = std::io::stdout().flush();
                    idx = (idx + 1) % frames.len();
                    tokio::time::sleep(Duration::from_millis(100)).await;
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
}
