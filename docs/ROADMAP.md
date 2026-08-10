# DIAD Roadmap

Status legend: `[x]` done · `[~]` partially done · `[ ]` not started.

## Scope & versions (honest)

**v1.0 = an agent console, not a desktop OS.** It is a bootable Sway ISO
whose job is to run the DIAD agent (GUI + TUI) well: chat, tool calling,
sessions, multi-provider access. It is *not* a general-purpose desktop:
no calendar/mail/office apps, no file-manager parity, near-empty a11y and
i18n coverage. The audit's Parity 38/100 is the accurate number; do not
advertise more.

- **v1.0** (current) — agent console: everything in Phases 1–2 below.
- **v1.1** — desktop parity: mako notifications, cliphist history,
  gammastep night light, freedesktop trash, keyboard a11y traps, and
  per-feature regression tests. (Phase A7 of the audit closes the four
  cheapest parity wins.)
- **v1.2** — hardening & i18n: crash/health dashboard, parallel provider
  routing, AUR packaging (`diad-agent-bin`, `diad-os`), locale/RTL/CJK
  test suite, i18n string layer.
- **Post v1.2** — the "Future" section below.

Each version ships only when its gates are green; un-gated features stay
in the next version rather than delaying a release.

## Phase 1: Foundation — DONE

**Goal**: Bootable OS with agent surfaces (GUI + TUI) and multi-provider LLM access.

- [x] Rust workspace + build system (`Makefile`, `install.sh`, archiso profile)
- [x] GUI: React + Vite IDE (chat, tool-call cards, edit-diff cards, editor,
      terminal, agents hub) served by the daemon on `127.0.0.1:7860`
- [x] TUI: ratatui client (`--tui`) over the same WebSocket API
- [x] Daemon: tokio + axum, versioned WS protocol (`core/protocol.rs`)
- [x] Provider adapters: Anthropic, Gemini, Qwen (native) + OpenAI-compatible
      path covering OpenAI, DeepSeek, Kimi, Groq, Mistral, OpenRouter, Ollama,
      custom endpoints
- [x] Tool registry: files, hardware (wifi/audio/display/input/power),
      network, system (packages/services/processes/updates/security)
- [x] Auth: keyring vault (libsecret) — keys never on disk in plaintext
- [x] ISO build pipeline (`make iso`); GRUB → autologin → Sway → supervisor

## Phase 2: Intelligence — MOSTLY DONE

- [x] Intent router: `classify_input()` + offline routing for no-network use
- [x] Session persistence (`~/.diad/sessions/`, JSON)
- [x] Tool chaining in the agent loop (tool_use → tool_result → next turn)
- [x] Destructive-action confirmation (approve/reject flows in GUI + TUI)
- [x] Git-aware file tree, `AgentDelegate` to external AI CLIs
- [~] Token usage / cost tracking (stats snapshot exists; per-provider cost
      ledger not yet implemented)
- [x] Provider failover on API error
- [ ] Session list view / search in the UI

## Phase 3: Multi-Provider & Polish — PARTIAL

- [x] Model picker: `F6` + `/model` across all providers
- [x] Custom OpenAI-compatible providers via config
- [x] Themes via `config.set` (theme key)
- [~] Toast/notification system (ToastStack in GUI)
- [ ] Parallel provider queries / consensus routing
- [ ] Crash recovery and health dashboard
- [ ] AUR packaging (`diad-agent-bin`, `diad-os`)

## Phase 4: Hardening (in progress)

- [x] Codebase sanitization: dead files, dead code, unused deps removed
- [x] Module restructure: `core/`, `tools/{files,hardware,network,system}`,
      `frontend/{components,panels,services,hooks,lib}`
- [x] Tests split by domain + sandbox negative test suite (328 unit + 39 int)
- [x] Docs: architecture, security, testing, audits (`docs/`)
- [~] Gate: fmt/clippy/build/test all clean (release build 0 warnings)
- [x] Release tag `v0.9-pre-gui` checkpoint

## Future (Post v1.2)

- On-device AI via Ollama/llama.cpp with hybrid cloud/local routing
- Voice interface (Whisper STT + TTS)
- WASM plugin runtime + community tool marketplace
- Scheduled/event-driven automation
- macOS/NixOS/Docker deployment targets
