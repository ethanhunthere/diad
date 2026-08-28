# `agent/` — The Brain

This is the **DIAD AGENT daemon** — the background program that does the real
work. Written in Rust, it runs headless on your machine, talks to cloud AI
providers, and performs actions through a sandboxed toolbox.

- It serves the GUI at `http://127.0.0.1:7860` and also offers a keyboard-only
  TUI (`diad-agent --tui`).
- It stores API keys in your system keyring, never in plain text.

## Folder map

| Path | What it does |
|------|--------------|
| `src/main.rs` | Entry point |
| `src/core/` | The heart: daemon, local web API, idle watcher, notes, terminal bridge |
| `src/orchestrator/` | Routes your intent and manages conversations |
| `src/providers/` | One adapter per AI company (Anthropic, Gemini, OpenAI, Qwen, …) |
| `src/tools/` | The sandboxed actions: files, hardware, network, system |
| `src/auth/` | OAuth and the secure key vault |
| `src/tui/` | The keyboard-first terminal interface |
| `tests/` | Automated checks |

Build with `make build-release` (from the repo root). See [`../STRUCTURE.md`](../STRUCTURE.md)
for the full picture.
