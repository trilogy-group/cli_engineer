# CLI Engineer Architecture Documentation

## Overview

CLI Engineer is an autonomous coding agent built with a sophisticated agentic architecture that follows a plan-execute-review cycle. The system is designed around an event-driven architecture with pluggable components that work together to interpret user tasks, create execution plans, execute those plans, and review the results.

### Core Architectural Principles

1. **Agentic Loop**: The heart of the system is an iterative loop that continuously refines its approach based on feedback
2. **Event-Driven Communication**: Components communicate through a centralized event bus
3. **Pluggable Providers**: Multiple LLM providers can be configured and swapped
4. **Context Management**: Intelligent context compression and management for long conversations
5. **Artifact Management**: Structured creation and management of generated files
6. **Iterative Refinement**: The system learns from previous iterations to improve subsequent attempts

## System Components

### 1. Agentic Loop (`agentic_loop.rs`)

The central orchestrator that implements the core AI agent workflow:

```rust
pub struct AgenticLoop {
    interpreter: Interpreter,
    planner: Planner,
    executor: Executor,
    reviewer: Reviewer,
    llm_manager: Arc<LLMManager>,
    max_iterations: usize,
    event_bus: Arc<EventBus>,
    // ... other fields
}
```

**Responsibilities:**
- Coordinates the interpret → plan → execute → review cycle
- Manages iteration context between cycles
- Handles failure recovery and retry logic
- Emits progress events throughout execution

**Key Methods:**
- `run()`: Main execution loop
- `post_process_artifacts()`: Cleanup and organization of generated files

### 2. LLM Manager (`llm_manager.rs`)

Manages communication with Large Language Models and abstracts provider differences:

```rust
pub struct LLMManager {
    providers: Vec<Box<dyn LLMProvider>>,
    event_bus: Option<Arc<EventBus>>,
    config: Option<Arc<Config>>,
}
```

**Responsibilities:**
- Route requests to active LLM providers
- Handle API rate limiting and retries
- Calculate token usage and costs
- Emit API call metrics

**Supported Providers:**
- OpenAI (GPT-4, GPT-4 Turbo, etc.)
- Anthropic (Claude models)
- OpenRouter (Various models)
- Local Provider (fallback for testing)

### 3. Task Interpreter (`interpreter.rs`)

Converts natural language input into structured task representations:

```rust
pub struct Task {
    pub description: String,
    pub goal: String,
}
```

**Responsibilities:**
- Parse user input into actionable tasks
- Extract intent and goals from natural language
- Categorize task types (code, refactor, review, docs, security)

### 4. Planner (`planner.rs`)

Creates structured execution plans from interpreted tasks:

```rust
pub struct Plan {
    pub goal: String,
    pub steps: Vec<Step>,
    pub dependencies: HashMap<String, Vec<String>>,
    pub estimated_complexity: ComplexityLevel,
}

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

**Step Categories:**
- `Analysis`: Understanding requirements, analyzing code
- `FileOperation`: Creating, reading, updating files
- `CodeGeneration`: Writing new code
- `CodeModification`: Modifying existing code
- `Testing`: Creating tests and validation
- `Documentation`: Writing docs or comments
- `Research`: Looking up APIs, best practices
- `Review`: Code review and quality checks

**Complexity Levels:**
- `Simple`: 1-3 steps, straightforward changes
- `Medium`: 4-10 steps, moderate complexity
- `Complex`: 10+ steps or high interdependency

### 5. Executor (`executor.rs`)

Executes planned steps and manages artifact creation:

```rust
pub struct Executor {
    artifact_manager: Option<Arc<ArtifactManager>>,
    context_manager: Option<Arc<ContextManager>>,
    event_bus: Option<Arc<EventBus>>,
    llm_manager: Arc<LLMManager>,
}

pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub output: String,
    pub artifacts_created: Vec<String>,
    pub tokens_used: usize,
    pub error: Option<String>,
}
```

**Responsibilities:**
- Execute individual plan steps
- Extract code artifacts from LLM responses
- Manage file creation and updates
- Handle execution errors and retries
- Track resource usage

### 6. Reviewer (`reviewer.rs`)

Evaluates execution results for quality and completeness:

```rust
pub struct ReviewResult {
    pub overall_quality: QualityLevel,
    pub issues: Vec<Issue>,
    pub suggestions: Vec<Suggestion>,
    pub ready_to_deploy: bool,
    pub summary: String,
}

pub struct Issue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub description: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
}
```

**Quality Levels:**
- `Excellent`: No issues, follows best practices
- `Good`: Minor issues or improvements possible
- `Fair`: Some issues that should be addressed
- `Poor`: Major issues requiring rework

**Issue Severities:**
- `Critical`: Must fix before proceeding
- `Major`: Should fix for quality
- `Minor`: Nice to fix but not blocking
- `Info`: Informational only

### 7. Context Manager (`context.rs`)

Manages conversation context and handles token limits:

```rust
pub struct ContextManager {
    config: ContextConfig,
    contexts: Arc<RwLock<HashMap<String, ConversationContext>>>,
    cache: Arc<RwLock<HashMap<String, CompressedContext>>>,
    event_bus: Option<Arc<EventBus>>,
    llm_manager: Option<Arc<LLMManager>>,
}
```

**Key Features:**
- Automatic context compression when approaching token limits
- Preservation of system messages (codebase files)
- Intelligent summarization of conversation history
- Context caching for performance

### 8. Artifact Manager (`artifact.rs`)

Manages creation and organization of generated files:

```rust
pub struct ArtifactManager {
    artifact_dir: PathBuf,
    artifacts: Arc<RwLock<Vec<Artifact>>>,
    event_bus: Option<Arc<EventBus>>,
}

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

**Artifact Types:**
- `SourceCode`: Programming language files
- `Configuration`: Config files (TOML, JSON, YAML)
- `Documentation`: Markdown and text files
- `Test`: Test files
- `Build`: Build scripts and files
- `Script`: Shell and automation scripts
- `Data`: Data files

### 9. Event Bus (`event_bus.rs`)

Provides event-driven communication between components:

```rust
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    metrics: Arc<RwLock<Metrics>>,
}
```

**Event Categories:**
- Task events (started, progress, completed, failed)
- Artifact events (created, updated)
- Execution events (started, progress, completed)
- Context events (usage, compression, cleared)
- API events (call started, completed, error)
- System events (config loaded, ready, shutdown)

### 10. Configuration (`config.rs`)

Manages system configuration and provider settings:

```rust
pub struct Config {
    pub ai_providers: AIProvidersConfig,
    pub execution: ExecutionConfig,
    pub ui: UIConfig,
    pub context: ContextConfig,
}
```

**Configuration Sections:**
- AI Providers: Model settings, API keys, costs
- Execution: Iteration limits, parallelism, artifacts
- UI: Display preferences, colors, progress bars
- Context: Token limits, compression, caching

### 11. User Interface Components

**Dashboard UI (`ui_dashboard.rs`):**
- Real-time dashboard with metrics
- Non-scrolling, compact display
- Live progress tracking
- Event-driven updates

**Enhanced UI (`ui_enhanced.rs`):**
- Rich terminal interface with colors
- Progress bars and animations
- Detailed metrics and summaries
- Session summaries

## Data Flow Architecture

### 1. Initialization Flow

```
main() → Config Loading → Provider Setup → Event Bus Creation → Manager Initialization
```

1. **Configuration Loading**: Load from `cli_engineer.toml` or defaults
2. **Provider Initialization**: Set up enabled LLM providers
3. **Manager Setup**: Create artifact, context, and LLM managers
4. **Event Bus**: Initialize event communication system
5. **UI Initialization**: Start appropriate user interface

### 2. Task Execution Flow

```
User Input → Interpreter → Planner → Executor → Reviewer → Decision Point
                ↑                                                    ↓
              Iterate ←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←← Continue?
```

**Detailed Flow:**

1. **Input Processing**:
   - User provides natural language task description
   - Optional codebase scanning for context
   - Context population with existing files

2. **Task Interpretation**:
   - Parse natural language into structured `Task`
   - Extract goals and requirements
   - Determine task category

3. **Planning Phase**:
   - Analyze task requirements
   - Create structured execution plan
   - Categorize steps by type
   - Estimate complexity and resources

4. **Execution Phase**:
   - Execute each step sequentially
   - Generate prompts based on step category
   - Call LLM providers for step execution
   - Extract and save artifacts
   - Track progress and metrics

5. **Review Phase**:
   - Analyze execution results
   - Identify issues and quality problems
   - Generate improvement suggestions
   - Determine if task is complete

6. **Decision Point**:
   - If ready to deploy: Complete successfully
   - If issues found: Create new iteration context
   - If max iterations reached: Report failure

### 3. Context Management Flow

```
New Message → Token Estimation → Context Check → Compression? → Storage
                                       ↓              ↓
                                   Under Limit    Over Threshold
                                       ↓              ↓
                                   Add Message    Compress Context
```

**Context Compression Process:**
1. Separate system messages (codebase files) from conversation
2. Identify recent messages to preserve
3. Summarize older messages using LLM
4. Rebuild context with summary + recent messages
5. Update token counts and emit events

### 4. Artifact Creation Flow

```
LLM Response → Artifact Extraction → Type Detection → File Creation → Manifest Update
```

**Artifact Extraction Process:**
1. Parse LLM response for XML artifact blocks
2. Extract filename, type, and content
3. Validate content and detect file type
4. Create file in artifact directory
5. Update artifact manifest
6. Emit artifact creation events

### 5. Event Flow

```
Component Action → Event Emission → Event Bus → Subscribers → UI Updates/Metrics
```

**Event Types and Flow:**
- **Task Events**: Progress tracking and status updates
- **API Events**: Usage metrics and cost tracking
- **Artifact Events**: File creation and updates
- **Context Events**: Memory management and compression
- **System Events**: Configuration and lifecycle

## Integration Points

### 1. Provider Integration

New LLM providers implement the `LLMProvider` trait:

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    fn context_size(&self) -> usize;
    async fn send_prompt(&self, prompt: &str) -> Result<String>;
    fn model_name(&self) -> &str;
}
```

### 2. UI Integration

UI components implement the `EventEmitter` trait:

```rust
#[async_trait]
pub trait EventEmitter {
    fn set_event_bus(&mut self, bus: Arc<EventBus>);
    async fn emit_event(&self, event: Event) -> Result<()>;
}
```

### 3. Command Integration

New commands are added to the `CommandKind` enum and handled in `main()`:

```rust
#[derive(ValueEnum, Debug, Clone)]
enum CommandKind {
    Code,     // Code generation
    Refactor, // Refactoring
    Review,   // Code review
    Docs,     // Documentation
    Security, // Security analysis
}
```

## Performance Considerations

### 1. Context Management

- **Compression Strategy**: Preserve recent messages while summarizing older content
- **Token Budgeting**: Dynamic allocation based on LLM context limits
- **Cache Utilization**: Disk-based caching for context persistence

### 2. API Optimization

- **Cost Tracking**: Real-time monitoring of API usage costs
- **Provider Selection**: Automatic failover between providers
- **Batch Processing**: Efficient handling of multiple API calls

### 3. Memory Management

- **Artifact Streaming**: Large files handled with streaming I/O
- **Context Compression**: Automatic compression when approaching limits
- **Event Buffer Management**: Bounded channels prevent memory leaks

## Security Considerations

### 1. API Key Management

- Environment variable storage for API keys
- No API keys in configuration files
- Secure transmission to providers

### 2. File System Access

- Sandboxed artifact directory
- Path validation for created files
- No arbitrary file system access

### 3. Code Execution

- No automatic code execution by default
- Isolated execution environment option
- User confirmation for potentially dangerous operations

## Extensibility

### 1. Adding New Providers

1. Implement `LLMProvider` trait
2. Add provider configuration to `Config`
3. Initialize in `setup_managers()`
4. Update documentation

### 2. Adding New Step Categories

1. Add variant to `StepCategory` enum
2. Update planner categorization logic
3. Add executor handling for new category
4. Update reviewer criteria

### 3. Adding New Commands

1. Add variant to `CommandKind` enum
2. Implement command logic in `main()`
3. Add command-specific prompts
4. Update CLI help text

This architecture provides a robust, extensible foundation for autonomous coding agents while maintaining clear separation of concerns and enabling easy testing and maintenance.