# ADR 001: Session and Context Orchestration

## Status
Proposed

## Context
`yuiclaw` wraps multiple independent AI agent CLIs (`gemini-cli`, `claude`, etc.). To provide a unified experience, the system must maintain session continuity and context awareness even when switching between different tools. This requires a central logic layer that sits between the communication interfaces (`acomm`) and the tools themselves.

## Decision
Establish **`acore`** as the central orchestration module for `yuiclaw`.

### 1. Responsibility of `acore`
- **Session Management:** Tracks the lifecycle of a conversation across different tool invocations.
- **Context Synthesis:** Queries `amem` for relevant facts and assembles a "System Context" to be injected into the tools.
- **Tool Routing:** Manages which CLI is best suited for the current task.
- **Transcript Normalization:** Standardizes CLI output into `Activity` logs for `amem`.

### 2. Implementation Strategy
- Implement as a Rust library/binary that integrates with `acomm` and `abeat`.
- Focus on subprocess management and stream transformation.

## Consequences
- Enables a tool-agnostic context layer.
- Simplifies `acomm` by removing tool execution logic.
