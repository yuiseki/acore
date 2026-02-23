# acore

Agent Core library for stateful AI CLI orchestration.

`acore` is the brain of the `yuiclaw` project, providing a uniform abstraction over AI agent CLIs (Gemini, Claude, Codex, OpenCode) to maintain conversation context, handle real-time streaming, and integrate with `amem` for persistent memory.

- **Stateful Session Management**: Automatically extracts and resumes sessions using CLI-specific flags.
- **Chunk-based Streaming**: Reads stdout in 1 KiB chunks for instantaneous feedback.
- **Memory Integration**: Dynamically fetches context from `amem` to enrich every session seed.
- **Pure CLI Wrapper**: Directly controls official CLI tools without relying on REST APIs.

## Architecture

```mermaid
flowchart TD
    Client["Caller (e.g. acomm bridge)"]
    SM["SessionManager\n(Arc<Mutex<HashMap>>)"]
    AE["AgentExecutor"]
    amem["amem CLI"]
    CLI["AI CLI\n(gemini / claude / codex / opencode)"]

    Client -->|execute_with_resume| SM
    SM -->|"first call: build_init_prompt()"| AE
    AE -->|"amem today --json"| amem
    amem --> AE
    AE -->|seed turn (JSON output)| CLI
    CLI -->|session_id| SM
    SM -->|resume turn (streaming)| CLI
    CLI -->|chunks| Client

    Client -->|execute_stream| AE
    AE -->|direct invocation| CLI
```

Roles of each component:

- `SessionManager` — maintains a `HashMap<AgentTool, session_id>` shared across threads (via `Arc<Mutex>`). On the first call for a given tool it seeds a new session, injecting the amem context snapshot. Subsequent calls resume the existing session.
- `AgentExecutor` — stateless helper for one-shot streaming execution and amem integration.
- `AgentTool` — enum with variants `Gemini`, `Claude`, `Codex`, `OpenCode`, `Mock`. Implements `Clone`, `Hash`, `Eq`, `Serialize`, `Deserialize`.

## Supported Tools

| Tool | Command | Session seed flags | Resume flags |
|---|---|---|---|
| `Gemini` | `gemini` | `--approval-mode yolo --output-format json -p <prompt>` | `--resume <id> -p <prompt>` |
| `Claude` | `claude` | `--dangerously-skip-permissions --output-format json --print <prompt>` | `--resume <id> --print <prompt>` |
| `Codex` | `codex` | `<prompt>` | `<prompt>` (stateless) |
| `OpenCode` | `opencode` | `<prompt>` | `<prompt>` (stateless) |
| `Mock` | — | (in-process echo) | — |

> **Note:** Codex and OpenCode do not expose a session resume flag at the CLI level; `acore` treats each call as stateless for those tools.

## Usage

### SessionManager — stateful resume

```rust
use acore::{SessionManager, AgentTool};

let manager = SessionManager::new();

// First call seeds the session with amem context and captures the session_id.
// Subsequent calls resume that session automatically.
manager.execute_with_resume(AgentTool::Gemini, "Hello", |chunk| {
    print!("{}", chunk);
}).await?;
```

### AgentExecutor — stateless streaming

```rust
use acore::{AgentExecutor, AgentTool};

AgentExecutor::execute_stream(AgentTool::Claude, "Summarise the repo", |chunk| {
    print!("{}", chunk);
}).await?;
```

### Memory helpers

```rust
// Fetch profile + soul + activities + P0 memories from amem
let context = AgentExecutor::fetch_context().await;

// Build the standard init prompt (used by SessionManager on first turn)
let prompt = AgentExecutor::build_init_prompt().await;

// Summarise a transcript and record it as an amem activity entry
AgentExecutor::summarize_and_record(AgentTool::Gemini, &transcript).await?;
```

## Technical Details

### Session ID Extraction

On the first turn (`Seed`), `acore` invokes the CLI with JSON output flags. The resulting JSON is scanned for:

1. `"session_id"` (snake_case — Gemini)
2. `"sessionId"` (camelCase — Claude)

### Resume Mechanism

Subsequent turns pass the captured ID via tool-specific flags:

- Gemini: `gemini --approval-mode yolo --resume <id> -p <prompt>`
- Claude: `claude --dangerously-skip-permissions --resume <id> --print <prompt>`

### amem Context Injection

`build_init_prompt()` calls `amem today --json` and formats the result into a structured prompt containing:

- Owner profile
- Agent soul
- Recent activities
- P0 agent memories

This is sent as the seed prompt so every new session starts with full context.

## Development

```bash
cargo fmt
cargo test   # 39 unit tests
cargo build
```

### ADR

- [ADR 001: Session and Context Orchestration](docs/ADR/001-session-and-context-orchestration.md)
- [ADR 002: Stateful Session Management](docs/ADR/002-stateful-session-management.md)
