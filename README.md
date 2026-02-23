# acore

Agent Core library for stateful AI CLI orchestration.

`acore` is the brain of the `yuiclaw` project, providing an abstraction layer over various AI agent CLIs (Gemini, Claude, etc.) to maintain conversation context and handle real-time streaming.

- **Stateful Session Management**: Automatically extracts and resumes sessions using CLI-specific flags.
- **Chunk-based Streaming**: Reads stdout byte-by-byte to provide instantaneous feedback.
- **Memory Integration**: Dynamically fetches context from `amem` to enrich agent prompts.
- **Pure CLI Wrapper**: Directly controls official CLI tools without relying on REST APIs.

## Supported Tools

- `gemini`: Google Gemini CLI.
- `claude`: Anthropic Claude Code CLI.
- `codex`: OpenAI Codex CLI.
- `opencode`: OpenCode CLI.

## Usage

### SessionManager

Manages IDs and execution flags for multiple tools.

```rust
use acore::{SessionManager, AgentTool};

let manager = SessionManager::new();
manager.execute_with_resume(AgentTool::Gemini, "Hello", |chunk| {
    print!("{}", chunk);
}).await?;
```

### AgentExecutor

Low-level execution and context fetching.

- `fetch_context()`: Retrieve profile and recent activities from `amem`.
- `summarize_and_record()`: Summarize session transcript and log to `amem`.

## Technical Details

### Session ID Extraction
On the first turn (Seed), `acore` invokes the CLI with JSON output flags to capture the `session_id`.

### Resume Mechanism
Subsequent turns use tool-specific resume flags:
- Gemini: `--resume <id> -p <prompt>`
- Claude: `--resume <id> --print <prompt>`

## Development

```bash
cargo fmt
cargo test
cargo build
```
