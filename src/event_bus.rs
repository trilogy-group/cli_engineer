use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// Events that can be emitted by components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    LogLine {
        level: String,
        message: String,
    },
    // Task-related events
    TaskStarted {
        task_id: String,
        description: String,
    },
    TaskProgress {
        task_id: String,
        progress: f32,
        message: String,
    },
    TaskCompleted {
        task_id: String,
        result: String,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },

    // Artifact events
    ArtifactCreated {
        name: String,
        path: String,
        artifact_type: String,
    },
    ArtifactUpdated {
        name: String,
        path: String,
    },

    // Execution events
    ExecutionStarted {
        environment: String,
    },
    ExecutionProgress {
        step: String,
        progress: f32,
    },
    ExecutionCompleted {
        output: String,
    },
    DependencyInstalling {
        package: String,
    },
    DependencyInstalled {
        package: String,
    },

    // Context events
    ContextUsage {
        used: usize,
        total: usize,
        percentage: f32,
    },
    ContextCompression {
        original_size: usize,
        compressed_size: usize,
    },
    ContextUsageChanged {
        id: String,
        usage_percentage: f32,
        total_tokens: usize,
    },
    ContextCompressed {
        id: String,
        original_tokens: usize,
        compressed_tokens: usize,
    },
    ContextCleared {
        id: String,
    },
    ContextCreated {
        id: String,
    },
    ContextCached {
        id: String,
    },
    ContextLoaded {
        id: String,
    },

    // API events
    APICallStarted {
        provider: String,
        model: String,
    },
    APICallCompleted {
        provider: String,
        tokens: usize,
        cost: f32,
    },
    APIError {
        provider: String,
        error: String,
    },

    // System events
    ConfigLoaded {
        path: Option<String>,
    },
    SystemReady,
    ShutdownRequested,

    // LLM events
    ReasoningTrace {
        message: String,
    },

    // Custom events
    Custom {
        event_type: String,
        data: serde_json::Value,
    },
}

/// Event bus for component communication
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    metrics: Arc<RwLock<Metrics>>,
}

/// Accumulated metrics from events
#[derive(Debug, Default, Clone)]
pub struct Metrics {
    pub total_api_calls: usize,
    pub total_tokens: usize,
    pub total_cost: f32,
    pub artifacts_created: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub current_context_usage: f32,
}

impl EventBus {
    /// Create a new event bus with specified channel capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            metrics: Arc::new(RwLock::new(Metrics::default())),
        }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Emit an event to all subscribers
    pub async fn emit(&self, event: Event) -> Result<()> {
        // Update metrics based on event
        self.update_metrics(&event).await;

        // Send event to subscribers
        match self.sender.send(event) {
            Ok(_) => Ok(()),
            Err(_) => {
                // No receivers, but that's okay
                Ok(())
            }
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }

    /// Update metrics based on event
    async fn update_metrics(&self, event: &Event) {
        let mut metrics = self.metrics.write().await;

        match event {
            Event::APICallCompleted { tokens, cost, .. } => {
                metrics.total_api_calls += 1;
                metrics.total_tokens += tokens;
                metrics.total_cost += cost;
            }
            Event::ArtifactCreated { .. } => {
                metrics.artifacts_created += 1;
            }
            Event::TaskCompleted { .. } => {
                metrics.tasks_completed += 1;
            }
            Event::TaskFailed { .. } => {
                metrics.tasks_failed += 1;
            }
            Event::ContextUsage { percentage, .. } => {
                metrics.current_context_usage = *percentage;
            }
            _ => {}
        }
    }
}

/// Trait for components that can emit events
#[async_trait::async_trait]
pub trait EventEmitter {
    fn set_event_bus(&mut self, bus: Arc<EventBus>);

    #[allow(dead_code)]
    async fn emit_event(&self, event: Event) -> Result<()>;
}

/// Helper macro to implement EventEmitter trait
#[macro_export]
macro_rules! impl_event_emitter {
    ($type:ty) => {
        #[async_trait::async_trait]
        impl EventEmitter for $type {
            fn set_event_bus(&mut self, bus: Arc<EventBus>) {
                self.event_bus = Some(bus);
            }

            async fn emit_event(&self, event: Event) -> Result<()> {
                if let Some(bus) = &self.event_bus {
                    bus.emit(event).await
                } else {
                    Ok(())
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_emission() {
        let bus = EventBus::new(100);
        let mut receiver = bus.subscribe();

        let event = Event::TaskStarted {
            task_id: "test-1".to_string(),
            description: "Test task".to_string(),
        };

        bus.emit(event.clone()).await.unwrap();

        let received = receiver.recv().await.unwrap();
        match received {
            Event::TaskStarted { task_id, .. } => {
                assert_eq!(task_id, "test-1");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let bus = EventBus::new(100);

        bus.emit(Event::APICallCompleted {
            provider: "openai".to_string(),
            tokens: 100,
            cost: 0.01,
        })
        .await
        .unwrap();

        let metrics = bus.get_metrics().await;
        assert_eq!(metrics.total_api_calls, 1);
        assert_eq!(metrics.total_tokens, 100);
        assert_eq!(metrics.total_cost, 0.01);
    }
}
