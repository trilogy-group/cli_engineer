use anyhow::Result;
use log::info;

/// Placeholder UI handler.
pub struct UIHandler {
    pub headless: bool,
}

impl UIHandler {
    pub fn new(headless: bool) -> Self { Self { headless } }

    pub fn start(&self) -> Result<()> {
        if !self.headless {
            info!("Starting UI");
        }
        Ok(())
    }
}
