# CLI Engineer API Reference

This document provides comprehensive API documentation for all core modules in the CLI Engineer codebase.

## Table of Contents

1. [LLM Providers](#llm-providers)
   - [OpenAI Provider](#openai-provider)
   - [Anthropic Provider](#anthropic-provider)
   - [OpenRouter Provider](#openrouter-provider)
   - [Ollama Provider](#ollama-provider)
   - [LLM Manager](#llm-manager)
2. [Core Modules](#core-modules)
   - [Planner](#planner)
   - [Executor](#executor)
   - [Reviewer](#reviewer)
   - [Interpreter](#interpreter)
3. [UI Components](#ui-components)
   - [Dashboard UI](#dashboard-ui)
   - [Enhanced UI](#enhanced-ui)
   - [Basic UI](#basic-ui)
4. [Supporting Systems](#supporting-systems)
   - [Event Bus](#event-bus)
   - [Context Manager](#context-manager)
   - [Artifact Manager](#artifact-manager)
   - [Configuration](#configuration)

---

## LLM Providers

### OpenAI Provider

The OpenAI provider implements the `LLMProvider` trait to interface with OpenAI's GPT models.

#### Configuration

```rust
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: usize,
    temperature: f32,
}
```

#### Methods

##### `new(model: Option<String>, temperature: Option<f32>) -> Result<Self>`

Creates a new OpenAI provider instance. Requires `OPENAI_API_KEY` environment variable.

**Parameters:**
- `model`: Optional model name (defaults to "gpt-4.1")
- `temperature`: Optional temperature setting (defaults to 0.2)

**Returns:** `Result<OpenAIProvider>`

##### `with_config(api_key: String, model: String, max_tokens: usize) -> Self`

Creates a provider with custom configuration.

##### `with_base_url(self, base_url: String) -> Self`

Sets a custom base URL for API-compatible services.

##### `with_temperature(self, temperature: f32) -> Self`

Sets the temperature for response generation.

#### Context Sizes

The provider automatically determines context size based on model:
- `gpt-4o`, `gpt-4o-mini`: 128,000 tokens
- `gpt-4-turbo`: 128,000 tokens
- `gpt-4`: 8,192 tokens
- `gpt-3.5-turbo`: 16,385 tokens
- Default: 4,096 tokens

#### Example Usage

```rust
let provider = OpenAIProvider::new(
    Some("gpt-4o".to_string()),
    Some(0.7)
)?;

let response = provider.send_prompt("Hello, world!").await?;
```

### Anthropic Provider

Implements Claude API integration following the same `LLMProvider` trait.

#### Configuration

```rust
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: Client,
    max_tokens: usize,
    temperature: f32,
}
```

#### Methods

##### `new(model: Option<String>, temperature: Option<f32>) -> Result<Self>`

Creates a new Anthropic provider. Requires `ANTHROPIC_API_KEY` environment variable.

**Parameters:**
- `model`: Optional model name (defaults to "claude-opus-4-0")
- `temperature`: Optional temperature setting (defaults to 0.2)

##### `with_model(self, model: String) -> Self`

Sets the Claude model to use.

##### `with_temperature(self, temperature: f32) -> Self`

Sets response generation temperature.

#### Supported Models

- `claude-3-opus-20240229`: 200,000 tokens
- `claude-3-sonnet-20240229`: 200,000 tokens
- `claude-3-haiku-20240307`: 200,000 tokens
- `claude-2.1`: 100,000 tokens
- `claude-2.0`: 100,000 tokens

#### Example Usage

```rust
let provider = AnthropicProvider::new(
    Some("claude-3-opus-20240229".to_string()),
    Some(0.5)
)?;

let response = provider.send_prompt("Analyze this code").await?;
```

### OpenRouter Provider

Provides access to multiple LLM models through the OpenRouter API.

#### Configuration

```rust
pub struct OpenRouterProvider {
    model: String,
    temperature: f32,
    max_tokens: usize,
    api_key: String,
    client: reqwest::Client,
}
```

#### Methods

##### `new(model: Option<String>, temperature: Option<f32>, max_tokens: Option<usize>) -> Result<Self>`

Creates a new OpenRouter provider. Requires `OPENROUTER_API_KEY` environment variable.

**Parameters:**
- `model`: Optional model name (defaults to "deepseek/deepseek-r1-0528-qwen3-8b")
- `temperature`: Optional temperature (defaults to 0.2)
- `max_tokens`: Optional max tokens (defaults to 8192)

#### Example Usage

```rust
let provider = OpenRouterProvider::new(
    Some("anthropic/claude-2".to_string()),
    Some(0.3),
    Some(4096)
)?;
```

### Ollama Provider

Provides access to local LLM models through Ollama's OpenAI-compatible API.

#### Configuration

```rust
pub struct OllamaProvider {
    model: String,
    base_url: String,
    client: Client,
    max_tokens: usize,
    temperature: f32,
}
```

#### Methods

##### `new(model: Option<String>, temperature: Option<f32>, base_url: Option<String>, max_tokens: Option<usize>) -> Result<Self>`

Creates a new Ollama provider. No API key required as Ollama runs locally.

**Parameters:**
- `model`: Optional model name (defaults to "qwen3:8b")
- `temperature`: Optional temperature (defaults to 0.7)
- `base_url`: Optional base URL (defaults to "http://localhost:11434")
- `max_tokens`: Optional max tokens (defaults to 8192)

##### `with_config(model: String, base_url: String, max_tokens: usize) -> Self`

Creates a provider with custom configuration.

##### `with_temperature(self, temperature: f32) -> Self`

Sets response generation temperature.

##### `with_model(self, model: String) -> Self`

Sets the model to use.

##### `with_base_url(self, base_url: String) -> Self`

Sets the Ollama server base URL.

#### Supported Models

**Recommended Models for Consumer GPUs (4B-14B parameters):**

**General Purpose (Best Balance):**
- `qwen3:4b` - 4B params, ~3GB VRAM, 128K context - Excellent for most tasks
- `qwen3:8b` - 8B params, ~6GB VRAM, 128K context - **Recommended default**
- `qwen3:14b` - 14B params, ~10GB VRAM, 128K context - High performance

**Reasoning & Advanced Tasks:**
- `deepseek-r1:7b` - 7B params, ~5GB VRAM, 131K context - Advanced reasoning
- `deepseek-r1:8b` - 8B params, ~6GB VRAM, 131K context - Reasoning + general

**Efficient & Compact:**
- `phi4-mini` - 3.8B params, ~3GB VRAM, 128K context - Microsoft's efficient model
- `gemma3:4b` - 4B params, ~3GB VRAM, 128K context - Google's compact model
- `gemma3:12b` - 12B params, ~8GB VRAM, 128K context - Stronger performance

**Code Specialized:**
- `qwen2.5-coder:7b` - 7B params, ~5GB VRAM, 32K context - Code generation
- `devstral` - 24B params, ~16GB VRAM, 128K context - **High-end GPUs only**

**Hardware Requirements:**
- **4B models**: 8GB+ VRAM (GTX 1070, RTX 3060, etc.)
- **7-8B models**: 12GB+ VRAM (RTX 3060 Ti, RTX 4060 Ti, etc.)
- **12-14B models**: 16GB+ VRAM (RTX 3080, RTX 4070 Ti, etc.)
- **24B+ models**: 24GB+ VRAM (RTX 3090, RTX 4090, etc.)

#### Example Usage

```rust
let provider = OllamaProvider::new(
    Some("qwen3:8b".to_string()),
    Some(0.7),
    Some("http://localhost:11434".to_string()),
    Some(8192)
)?;

let response = provider.send_prompt("Write a hello world function").await?;
```

#### Setup Requirements

1. Install Ollama: `curl -fsSL https://ollama.ai/install.sh | sh`
2. Pull a model: `ollama pull qwen3:8b` (recommended for most users)
3. Start Ollama server: `ollama serve` (usually runs automatically)
4. Configure in `cli_engineer.toml` - simply set `enabled = true`:

```toml
[ai_providers.ollama]
enabled = true
model = "qwen3:8b"  # Change to your preferred model
temperature = 0.7
base_url = "http://localhost:11434"
max_tokens = 8192
```

**Quick Model Selection Guide:**
- **8GB VRAM**: `qwen3:4b` or `phi4-mini`
- **12GB VRAM**: `qwen3:8b` or `deepseek-r1:7b` (recommended)
- **16GB+ VRAM**: `qwen3:14b` or `gemma3:12b`

### LLM Manager

Coordinates multiple LLM providers and manages context limits.

#### Configuration

```rust
pub struct LLMManager {
    providers: Vec<Box<dyn LLMProvider>>,
    event_bus: Option<Arc<EventBus>>,
    config: Option<Arc<Config>>,
}
```

#### Methods

##### `new(providers: Vec<Box<dyn LLMProvider>>, event_bus: Arc<EventBus>, config: Arc<Config>) -> Self`

Creates a new manager with specified providers.

##### `get_context_size(&self) -> usize`

Returns the context size of the active provider.

##### `send_prompt(&self, prompt: &str) -> Result<String>`

Sends a prompt to the first available provider and tracks usage metrics.

#### Example Usage

```rust
let providers: Vec<Box<dyn LLMProvider>> = vec![
    Box::new(OpenAIProvider::new(None, None)?),
    Box::new(AnthropicProvider::new(None, None)?),
];

let manager = LLMManager::new(providers, event_bus, config);
let response = manager.send_prompt("Generate code").await?;
```

---

## Core Modules

### Planner

The planner creates structured execution plans from user tasks.

#### Types

##### `Plan`

```rust
pub struct Plan {
    pub goal: String,
    pub steps: Vec<Step>,
    pub dependencies: HashMap<String, Vec<String>>,
    pub estimated_complexity: ComplexityLevel,
}
```

##### `Step`

```rust
pub struct Step {
    pub id: String,
    pub description: String,
    pub category: StepCategory,
    pub inputs: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub success_criteria: Vec<String>,
    pub estimated_tokens: usize,
}
```

##### `StepCategory`

Available step categories:
- `Analysis`: Understanding requirements, analyzing code
- `FileOperation`: Creating, reading, updating files
- `CodeGeneration`: Writing new code
- `CodeModification`: Modifying existing code
- `Testing`: Running tests or validation
- `Documentation`: Writing docs or comments
- `Research`: Looking up APIs, best practices
- `Review`: Code review and quality checks

##### `ComplexityLevel`

- `Simple`: 1-3 steps, straightforward changes
- `Medium`: 4-10 steps, moderate complexity
- `Complex`: 10+ steps or high interdependency

#### Methods

##### `new() -> Self`

Creates a new planner instance.

##### `plan(&self, task: &Task, llm_manager: &LLMManager, config: Option<&Config>, iteration_context: Option<&IterationContext>) -> Result<Plan>`

Creates a structured plan for the given task.

**Parameters:**
- `task`: The interpreted user task
- `llm_manager`: LLM manager for generating plans
- `config`: Optional configuration
- `iteration_context`: Context from previous iterations

**Returns:** `Result<Plan>`

#### Example Usage

```rust
let planner = Planner::new();
let task = Task {
    description: "Create a hello world program".to_string(),
    goal: "Generate working hello world code".to_string(),
};

let plan = planner.plan(&task, &llm_manager, Some(&config), None).await?;
```

### Executor

Executes planned steps and manages artifact creation.

#### Types

##### `StepResult`

```rust
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: String,
    pub artifacts_created: Vec<String>,
    pub tokens_used: usize,
    pub error: Option<String>,
}
```

#### Methods

##### `new(llm_manager: Arc<LLMManager>) -> Self`

Creates a new executor with the specified LLM manager.

##### `with_artifact_manager(self, manager: Arc<ArtifactManager>) -> Self`

Adds artifact management capability.

##### `with_context_manager(self, manager: Arc<ContextManager>) -> Self`

Adds context management capability.

##### `with_event_bus(self, bus: Arc<EventBus>) -> Self`

Enables event emission.

##### `execute(&self, plan: &Plan, context_id: &str) -> Result<Vec<StepResult>>`

Executes the entire plan and returns results for each step.

**Parameters:**
- `plan`: The execution plan
- `context_id`: Context identifier for message tracking

**Returns:** `Result<Vec<StepResult>>`

#### Step Execution Logic

The executor handles different step categories with specific prompts:

- **Analysis**: Provides analysis in text format only
- **FileOperation/CodeGeneration**: Creates files using XML artifact format
- **CodeModification**: Modifies existing code with diff-like output
- **Testing**: Creates test code without execution
- **Documentation**: Creates markdown files in docs/ directory
- **Research**: Provides research findings in text format

#### Example Usage

```rust
let executor = Executor::new(llm_manager)
    .with_artifact_manager(artifact_manager)
    .with_context_manager(context_manager)
    .with_event_bus(event_bus);

let results = executor.execute(&plan, "context-123").await?;
```

### Reviewer

Reviews execution results for quality and correctness.

#### Types

##### `ReviewResult`

```rust
pub struct ReviewResult {
    pub overall_quality: QualityLevel,
    pub issues: Vec<Issue>,
    pub suggestions: Vec<Suggestion>,
    pub ready_to_deploy: bool,
    pub summary: String,
}
```

##### `QualityLevel`

- `Excellent`: No issues, follows best practices
- `Good`: Minor issues or improvements possible
- `Fair`: Some issues that should be addressed
- `Poor`: Major issues requiring rework

##### `Issue`

```rust
pub struct Issue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub description: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
}
```

##### `IssueSeverity`

- `Critical`: Must fix before proceeding
- `Major`: Should fix for quality
- `Minor`: Nice to fix but not blocking
- `Info`: Informational only

##### `IssueCategory`

- `Logic`: Logic errors or bugs
- `Performance`: Performance concerns
- `Security`: Security vulnerabilities
- `CodeStyle`: Style and formatting
- `BestPractices`: Not following best practices
- `Documentation`: Missing or poor documentation
- `Testing`: Insufficient testing
- `Dependencies`: Dependency issues

#### Methods

##### `new() -> Self`

Creates a new reviewer instance.

##### `with_event_bus(self, bus: Arc<EventBus>) -> Self`

Enables event emission for review progress.

##### `review(&self, plan: &Plan, results: &[StepResult], llm_manager: &LLMManager, context_id: &str) -> Result<ReviewResult>`

Reviews execution results for correctness and quality.

**Parameters:**
- `plan`: The original execution plan
- `results`: Results from step execution
- `llm_manager`: LLM manager for generating reviews
- `context_id`: Context identifier

**Returns:** `Result<ReviewResult>`

#### Review Process

The reviewer:
1. Analyzes each step's output and artifacts
2. Checks for actual issues (not theoretical ones)
3. Provides specific feedback with severity levels
4. Determines if the results are ready for deployment
5. Suggests improvements where applicable

#### Example Usage

```rust
let reviewer = Reviewer::new().with_event_bus(event_bus);
let review = reviewer.review(&plan, &results, &llm_manager, "context-123").await?;

if review.ready_to_deploy {
    println!("✅ Ready to deploy!");
} else {
    println!("❌ Issues found: {}", review.issues.len());
}
```

### Interpreter

Converts raw user input into structured tasks.

#### Types

##### `Task`

```rust
pub struct Task {
    pub description: String,
    pub goal: String,
}
```

#### Methods

##### `new() -> Self`

Creates a new interpreter instance.

##### `interpret(&self, input: &str) -> Result<Task>`

Interprets user input into a structured task.

**Parameters:**
- `input`: Raw user input string

**Returns:** `Result<Task>`

#### Interpretation Logic

The interpreter analyzes input keywords to determine task type:
- "create", "build" → Creation task
- "fix", "debug" → Debugging task
- "test" → Testing task
- Default → General completion task

#### Example Usage

```rust
let interpreter = Interpreter::new();
let task = interpreter.interpret("Create a Python hello world script")?;

println!("Goal: {}", task.goal);
// Output: "Goal: Create or build: Create a Python hello world script"
```

---

## UI Components

### Dashboard UI

Provides a real-time, non-scrolling dashboard interface.

#### Configuration

```rust
pub struct DashboardUI {
    headless: bool,
    event_bus: Option<Arc<EventBus>>,
    start_time: Instant,
    // Various Arc<Mutex<>> fields for thread-safe state
}
```

#### Methods

##### `new(headless: bool) -> Self`

Creates a new dashboard UI instance.

##### `start(&mut self) -> Result<()>`

Initializes the dashboard and starts event listening.

##### `finish(&mut self) -> Result<()>`

Cleans up and shows final summary.

##### `update_status(&mut self, status: &str) -> Result<()>`

Updates the current status display.

##### `throttled_render(&mut self) -> Result<()>`

Renders the dashboard with throttling to prevent flickering.

#### Features

- Real-time progress tracking
- API call metrics
- Cost tracking
- Artifact creation monitoring
- Context usage display
- Live log streaming

#### Example Usage

```rust
let mut ui = DashboardUI::new(false);
ui.set_event_bus(event_bus);
ui.start()?;

// UI will automatically update based on events
// ...

ui.finish()?;
```

### Enhanced UI

Provides a colorful terminal interface with progress bars.

#### Configuration

```rust
pub struct EnhancedUI {
    headless: bool,
    multi_progress: MultiProgress,
    main_progress: Option<ProgressBar>,
    metrics_bar: Option<ProgressBar>,
    event_bus: Option<Arc<EventBus>>,
    start_time: Instant,
    last_metrics: Arc<RwLock<Metrics>>,
}
```

#### Methods

##### `new(headless: bool) -> Self`

Creates a new enhanced UI instance.

##### `start(&mut self) -> Result<()>`

Initializes progress bars and event handling.

##### `finish(&mut self)`

Shows session summary and cleans up.

##### `display_error(&mut self, error: &str) -> Result<()>`

Displays error messages with formatting.

#### Features

- Multiple progress bars
- Colored output
- Real-time metrics
- Session summaries
- Event-driven updates

#### Example Usage

```rust
let mut ui = EnhancedUI::new(false);
ui.set_event_bus(event_bus);
ui.start()?;

// Execute tasks...

ui.finish();
```

### Basic UI

Simple terminal UI with optional spinner.

#### Methods

##### `new(headless: bool) -> Self`

Creates a basic UI instance.

##### `start(&mut self) -> Result<()>`

Starts the UI with optional spinner.

##### `finish(&mut self)`

Stops spinner and cleans up.

##### `display_task(&mut self, task: &str) -> Result<()>`

Displays task information.

##### `display_error(&mut self, error: &str) -> Result<()>`

Displays error messages.

---

## Supporting Systems

### Event Bus

Provides event-driven communication between components.

#### Types

##### `Event`

```rust
pub enum Event {
    LogLine { level: String, message: String },
    TaskStarted { task_id: String, description: String },
    TaskProgress { task_id: String, progress: f32, message: String },
    TaskCompleted { task_id: String, result: String },
    TaskFailed { task_id: String, error: String },
    ArtifactCreated { name: String, path: String, artifact_type: String },
    APICallStarted { provider: String, model: String },
    APICallCompleted { provider: String, tokens: usize, cost: f32 },
    // ... and many more
}
```

##### `Metrics`

```rust
pub struct Metrics {
    pub total_api_calls: usize,
    pub total_tokens: usize,
    pub total_cost: f32,
    pub artifacts_created: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub current_context_usage: f32,
}
```

#### Methods

##### `new(capacity: usize) -> Self`

Creates a new event bus with specified channel capacity.

##### `subscribe(&self) -> broadcast::Receiver<Event>`

Subscribes to events.

##### `emit(&self, event: Event) -> Result<()>`

Emits an event to all subscribers.

##### `get_metrics(&self) -> Metrics`

Returns current accumulated metrics.

#### Example Usage

```rust
let event_bus = Arc::new(EventBus::new(1000));
let mut receiver = event_bus.subscribe();

// Emit events
event_bus.emit(Event::TaskStarted {
    task_id: "task-1".to_string(),
    description: "Generate code".to_string(),
}).await?;

// Receive events
while let Ok(event) = receiver.recv().await {
    match event {
        Event::TaskStarted { description, .. } => {
            println!("Started: {}", description);
        }
        _ => {}
    }
}
```

### Context Manager

Manages conversation context and token limits with automatic compression.

#### Configuration

```rust
pub struct ContextConfig {
    pub max_tokens: usize,
    pub compression_threshold: f32,
    pub cache_enabled: bool,
    pub cache_dir: PathBuf,
}
```

#### Methods

##### `new(config: ContextConfig) -> Result<Self>`

Creates a new context manager.

##### `create_context(&self, metadata: HashMap<String, String>) -> String`

Creates a new conversation context and returns its ID.

##### `add_message(&self, context_id: &str, role: String, content: String) -> Result<()>`

Adds a message to the context with automatic compression when needed.

##### `get_messages(&self, context_id: &str, max_tokens: Option<usize>) -> Result<Vec<Message>>`

Retrieves messages from context with optional token limiting.

#### Features

- Automatic context compression when approaching token limits
- LLM-generated summaries for compressed content
- Caching support
- Token usage tracking
- Separate handling of system vs conversation messages

#### Example Usage

```rust
let config = ContextConfig {
    max_tokens: 100_000,
    compression_threshold: 0.8,
    cache_enabled: true,
    cache_dir: PathBuf::from("./cache"),
};

let mut context_manager = ContextManager::new(config)?;
context_manager.set_llm_manager(llm_manager);

let context_id = context_manager.create_context(HashMap::new()).await;
context_manager.add_message(&context_id, "user".to_string(), "Hello".to_string()).await?;
```

### Artifact Manager

Manages creation, storage, and organization of generated artifacts.

#### Types

##### `ArtifactType`

```rust
pub enum ArtifactType {
    SourceCode,
    Configuration,
    Documentation,
    Test,
    Build,
    Script,
    Data,
    Other(String),
}
```

##### `Artifact`

```rust
pub struct Artifact {
    pub id: String,
    pub name: String,
    pub artifact_type: ArtifactType,
    pub path: PathBuf,
    pub content: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}
```

#### Methods

##### `new(artifact_dir: PathBuf) -> Result<Self>`

Creates a new artifact manager with specified directory.

##### `create_artifact(&self, name: String, artifact_type: ArtifactType, content: String, metadata: HashMap<String, String>) -> Result<Artifact>`

Creates a new artifact and saves it to disk.

##### `list_artifacts(&self) -> Vec<Artifact>`

Returns all managed artifacts.

##### `cleanup(&self) -> Result<()>`

Removes orphaned files not tracked in the manifest.

#### Features

- Automatic file extension detection
- Metadata tracking
- JSON manifest for persistence
- Parent directory creation
- Type-based organization

#### Example Usage

```rust
let artifact_manager = ArtifactManager::new(PathBuf::from("./artifacts"))?;

let mut metadata = HashMap::new();
metadata.insert("language".to_string(), "rust".to_string());

let artifact = artifact_manager.create_artifact(
    "hello_world".to_string(),
    ArtifactType::SourceCode,
    "fn main() { println!(\"Hello, world!\"); }".to_string(),
    metadata,
).await?;
```

### Configuration

Manages application configuration from TOML files.

#### Types

##### `Config`

```rust
pub struct Config {
    pub ai_providers: AIProvidersConfig,
    pub execution: ExecutionConfig,
    pub ui: UIConfig,
    pub context: ContextConfig,
}
```

##### `ProviderConfig`

```rust
pub struct ProviderConfig {
    pub enabled: bool,
    pub model: String,
    pub temperature: Option<f32>,
    pub cost_per_1m_input_tokens: Option<f32>,
    pub cost_per_1m_output_tokens: Option<f32>,
    pub max_tokens: Option<usize>,
}
```

#### Methods

##### `load(config_path: &Option<String>) -> Result<Self>`

Loads configuration from file or returns defaults.

##### `from_file<P: AsRef<Path>>(path: P) -> Result<Self>`

Loads configuration from a specific file.

##### `save<P: AsRef<Path>>(&self, path: P) -> Result<()>`

Saves configuration to a file.

#### Configuration Locations

The system searches for configuration in this order:
1. Explicitly specified path
2. `cli_engineer.toml` (current directory)
3. `.cli_engineer.toml` (current directory)
4. `~/.config/cli_engineer/config.toml` (user config)

#### Example Configuration

```toml
[ai_providers.openai]
enabled = true
model = "gpt-4o"
temperature = 0.7
cost_per_1m_input_tokens = 2.0
cost_per_1m_output_tokens = 8.0

[ai_providers.ollama]
enabled = true
model = "qwen3:8b"
temperature = 0.7
base_url = "http://localhost:11434"
max_tokens = 8192

[execution]
max_iterations = 10
parallel_enabled = true
artifact_dir = "./artifacts"

[ui]
colorful = true
progress_bars = true
output_format = "terminal"
```

---

## Integration Examples

### Complete Workflow

```rust
use cli_engineer::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Arc::new(Config::load(&None)?);
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(1000));
    
    // Setup providers
    let providers: Vec<Box<dyn LLMProvider>> = vec![
        Box::new(OpenAIProvider::new(None, None)?),
    ];
    
    // Create managers
    let llm_manager = Arc::new(LLMManager::new(providers, event_bus.clone(), config.clone()));
    let artifact_manager = Arc::new(ArtifactManager::new(PathBuf::from("./artifacts"))?);
    
    // Create and run agentic loop
    let agentic_loop = AgenticLoop::new(
        llm_manager.clone(),
        config.execution.max_iterations,
        event_bus.clone(),
    )
    .with_artifact_manager(artifact_manager);
    
    // Execute task
    let context_id = "main-context";
    agentic_loop.run("Create a hello world program", context_id).await?;
    
    Ok(())
}
```

### Custom Event Handling

```rust
let event_bus = Arc::new(EventBus::new(1000));
let mut receiver = event_bus.subscribe();

tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event {
            Event::ArtifactCreated { name, artifact_type, .. } => {
                println!("Created {} ({})", name, artifact_type);
            }
            Event::APICallCompleted { provider, tokens, cost } => {
                println!("API call to {} completed: {} tokens, ${:.4}", provider, tokens, cost);
            }
            _ => {}
        }
    }
});
```

This completes the comprehensive API reference documentation for the CLI Engineer codebase. Each module, type, and method is documented with usage examples and integration patterns.