# ADR 002: Stateful Session Management

## Status
Proposed

## Context
Currently, `acore` spawns a new CLI subprocess for every prompt, making it stateless. This results in the AI agent forgetting previous context within the same conversation. To provide a continuous experience, we need a way to keep the agent's mind (process) alive throughout a session.

## Decision
Implement a persistent process pool within `acore`.

### 1. Persistent Process Strategy
- Instead of short-lived `Command::output()`, use long-lived `tokio::process::Child`.
- Maintain a pool of active processes, indexed by `AgentTool`.
- Communication happens via `stdin` (for prompts) and `stdout` (for streaming responses).

### 2. Context Continuity
- By keeping the process alive, we leverage the CLI's native ability to maintain context (e.g., Gemini's internal session state or Claude's REPL mode).
- This avoids re-reading logs or re-injecting full context every time, saving tokens and improving latency.

### 3. Lifecycle Management
- Processes are kept alive until:
  - The tool is manually switched.
  - The user ends the session (`/clear` or exit).
  - A timeout or error occurs.

## Consequences
- **Memory:** The agent will remember previous turns in the same session.
- **Complexity:** Parsing the output of a long-running process requires reliable "end-of-turn" detection (which might vary by tool).
- **Efficiency:** Significant reduction in context re-processing overhead.
