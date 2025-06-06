use crate::event_bus::{Event, EventBus};
use log::{LevelFilter, Metadata, Record, SetLoggerError};
use std::sync::Arc;
use tokio;

pub struct DashboardLogger {
    pub event_bus: Arc<EventBus>,
    pub level: LevelFilter,
}

impl log::Log for DashboardLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level.to_level().unwrap_or(log::Level::Error)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!("{}", record.args());

            // Use tokio::spawn to handle the async emit
            let event_bus = self.event_bus.clone();
            let level = record.level().to_string();
            let message = msg.clone();

            tokio::spawn(async move {
                let _ = event_bus.emit(Event::LogLine { level, message }).await;
            });
        }
    }

    fn flush(&self) {}
}

impl DashboardLogger {
    pub fn init(event_bus: Arc<EventBus>, level: LevelFilter) -> Result<(), SetLoggerError> {
        let logger = Box::leak(Box::new(DashboardLogger { event_bus, level }));
        log::set_logger(logger)?;
        log::set_max_level(level);
        Ok(())
    }
}
