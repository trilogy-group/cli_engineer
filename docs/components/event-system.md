# Event System & Concurrency Documentation

## Overview

The CLI Engineer's event system provides a robust communication mechanism between components using an event bus pattern with built-in metrics tracking. The system is designed for asynchronous operation with concurrent task execution capabilities.

## Event Bus Architecture

### Core Components

- **EventBus**: Central message broker for component communication
- **Event**: Enumerated event types for different system operations
- **EventEmitter**: Trait for components that can emit events
- **Metrics**: Accumulated statistics from system events

### Event Types

The system supports various event categories:

#### Task Events
```rust
TaskStarted { task_id: String, description: String }
TaskProgress { task_id: String, progress: f32, message: String }
TaskCompleted { task_id: String, result: String }
TaskFailed { task_id: String, error: String }
```

#### Artifact Events
```rust
ArtifactCreated { name: String, path: String, artifact_type: String }
ArtifactUpdated { name: String, path: String }
```

#### API Events
```rust
APICallStarted { provider: String, model: String }
APICallCompleted { provider: String, tokens: usize, cost: f32 }
APIError { provider: String, error: String }
```

#### Context Events
```rust
ContextUsage { used: usize, total: usize, percentage: f32 }
ContextCompression { original_size: usize, compressed_size: usize }
ContextUsageChanged { id: String, usage_percentage: f32, total_tokens: usize }
```

#### System Events
```rust
ConfigLoaded { path: Option<String> }
SystemReady
ShutdownRequested
```

## Event Bus Implementation

### Creating an Event Bus

```rust
use cli_engineer::event_bus::EventBus;

// Create with specified channel capacity
let event_bus = Arc::new(EventBus::new(1000));
```

### Subscribing to Events

```rust
let mut receiver = event_bus.subscribe();

tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event {
            Event::TaskStarted { description, .. } => {
                println!("Task started: {}", description);
            }
            Event::APICallCompleted { tokens, cost, .. } => {
                println!("API call completed: {} tokens, ${:.4}", tokens, cost);
            }
            _ => {}
        }
    }
});
```

### Emitting Events

Components implement the EventEmitter trait:

```rust
use cli_engineer::event_bus::{Event, EventEmitter};

struct MyComponent {
    event_bus: Option<Arc<EventBus>>,
}

impl EventEmitter for MyComponent {
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
```

### Metrics Tracking

The event bus automatically tracks metrics from events:

```rust
// Get current metrics
let metrics = event_bus.get_metrics().await;

println!("Total API calls: {}", metrics.total_api_calls);
println!("Total cost: ${:.4}", metrics.total_cost);
println!("Artifacts created: {}", metrics.artifacts_created);
```

## Concurrency Support

### Parallel Task Execution

The concurrency module provides utilities for running tasks in parallel:

```rust
use cli_engineer::concurrency::run_parallel;

// Run multiple futures concurrently
let futures = vec![
    async { process_file("file1.rs").await },
    async { process_file("file2.rs").await },
    async { process_file("file3.rs").await },
];

let results = run_parallel(futures).await?;
```

### Async Event Handling

All event handling is asynchronous and non-blocking:

```rust
// Events are emitted asynchronously
tokio::spawn(async move {
    loop {
        // Periodic status update
        event_bus.emit(Event::TaskProgress {
            task_id: "main".to_string(),
            progress: calculate_progress(),
            message: "Processing...".to_string(),
        }).await?;
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
});
```

## Event Bus Configuration

### Channel Capacity

Configure the broadcast channel capacity based on expected event volume:

```rust
// High-throughput applications
let event_bus = EventBus::new(10_000);

// Low-throughput applications
let event_bus = EventBus::new(100);
```

### Error Handling

Events that fail to send (no receivers) are handled gracefully:

```rust
// This won't fail even if no one is listening
event_bus.emit(Event::SystemReady).await?;
```

## Integration with UI Components

### Dashboard Integration

The event bus integrates seamlessly with UI components:

```rust
// Dashboard listens for specific events
match event {
    Event::TaskProgress { progress, message, .. } => {
        dashboard.update_progress(progress, &message)?;
    }
    Event::APICallCompleted { cost, .. } => {
        dashboard.update_cost(cost)?;
    }
    _ => {}
}
```

### Real-time Updates

Events enable real-time UI updates without polling:

```rust
// UI components subscribe to relevant events
let mut receiver = event_bus.subscribe();
tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        ui.handle_event(event).await?;
    }
});
```

## Best Practices

### Event Design

1. **Specific Events**: Create specific event types rather than generic ones
2. **Immutable Data**: Event data should be immutable and cloneable
3. **Meaningful Names**: Use descriptive event names that indicate purpose

### Performance Considerations

1. **Channel Capacity**: Size channels appropriately for your use case
2. **Event Frequency**: Avoid high-frequency events that could overwhelm subscribers
3. **Async Processing**: Keep event handlers lightweight and non-blocking

### Error Handling

1. **Graceful Degradation**: Components should work even if event emission fails
2. **Logging**: Log important events for debugging and monitoring
3. **Retry Logic**: Implement retry logic for critical events if needed

## Testing

### Unit Testing Events

```rust
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
    // Assert event properties
}
```

### Integration Testing

Test event flow between components:

```rust
#[tokio::test]
async fn test_component_communication() {
    let event_bus = Arc::new(EventBus::new(100));
    
    let mut component_a = ComponentA::new();
    component_a.set_event_bus(event_bus.clone());
    
    let mut component_b = ComponentB::new();
    component_b.set_event_bus(event_bus.clone());
    
    // Test that A can communicate with B through events
}
```

## Troubleshooting

### Common Issues

1. **Events Not Received**: Ensure subscribers are created before events are emitted
2. **Memory Usage**: Monitor channel capacity and subscriber count
3. **Deadlocks**: Avoid blocking operations in event handlers

### Debugging

Enable event logging for debugging:

```rust
tokio::spawn(async move {
    let mut receiver = event_bus.subscribe();
    while let Ok(event) = receiver.recv().await {
        println!("Event: {:?}", event);
    }
});
```