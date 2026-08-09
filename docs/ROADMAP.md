# DIAD Roadmap

## Phase 1: Foundation (Weeks 1-4)

**Goal**: Bootable OS with basic TUI and one LLM provider.

### Week 1: Project Scaffolding
- [x] Initialize Rust project with all dependencies
- [x] Create directory structure and build system
- [x] Write Arch ISO profile and install scripts
- [ ] Verify ISO builds successfully

### Week 2: Core TUI
- [x] Implement TUI framework (app state, event loop, rendering)
- [x] Build chat view, file tree view, code view, system status view
- [x] Implement input handling with cursor movement
- [ ] Add syntax highlighting for code view
- [ ] Implement scrollback in chat view
- [ ] Add mouse support for mode switching

### Week 3: LLM Integration
- [x] Implement `LlmProvider` trait
- [x] Build Anthropic adapter with SSE streaming
- [x] Build OpenAI adapter with SSE streaming
- [x] Build Gemini adapter with SSE streaming
- [x] Build Qwen adapter with SSE streaming
- [ ] Add streaming display in TUI (character-by-character)
- [ ] Implement token usage tracking

### Week 4: Tool System & Auth
- [x] Build ToolRegistry with all system tools
- [x] Implement filesystem, process, network, and system tools
- [x] Build OAuth flow and keyring vault
- [ ] Wire tools into the LLM conversation loop
- [ ] Implement tool call parsing and execution
- [ ] Add destructive action confirmation

### Milestone: Alpha Release
- DIAD ISO boots into Sway
- DIAD Agent launches and displays TUI
- Can chat with at least one LLM provider
- Basic tool execution works

---

## Phase 2: Intelligence (Weeks 5-8)

**Goal**: Smart intent routing, session management, enhanced tooling.

### Week 5: Intent Router Enhancement
- [ ] Replace keyword heuristic with LLM-based classification
- [ ] Add context-aware intent parsing
- [ ] Implement multi-turn conversation state tracking
- [ ] Add automatic tool selection from user intent

### Week 6: Session Management
- [x] Session persistence (save/load JSON)
- [ ] Session list view in TUI
- [ ] Session search and filtering
- [ ] Session export (Markdown, HTML)
- [ ] Auto-save with configurable interval

### Week 7: Tool Enhancement
- [ ] Add more filesystem tools (move, copy, symlink)
- [ ] Add git integration (status, diff, commit, push)
- [ ] Add Docker/container management tools
- [ ] Add database query tools (SQLite, PostgreSQL)
- [ ] Implement tool chaining (pipe output between tools)

### Week 8: Provider Features
- [ ] Image/multimodal support (paste images into chat)
- [ ] Provider failover (auto-switch on API error)
- [ ] Rate limiting and retry with exponential backoff
- [ ] Cost tracking per provider per session

### Milestone: Beta Release
- Intelligent intent routing works reliably
- Sessions persist across restarts
- Multiple providers with failover
- Enhanced tool ecosystem

---

## Phase 3: Multi-Provider & Polish (Weeks 9-12)

**Goal**: Production-ready experience with multi-provider support.

### Week 9: Multi-Provider Orchestration
- [ ] Parallel provider queries (compare responses)
- [ ] Provider voting/consensus for critical actions
- [ ] Specialized provider routing (code → Claude, creative → GPT-4)
- [ ] Provider performance dashboard

### Week 10: UI Polish
- [ ] Custom themes (load from TOML)
- [ ] Smooth animations (typing indicator, transitions)
- [ ] Notification system (toast-style alerts)
- [ ] Progress indicators for long-running tools
- [ ] Accessibility improvements (screen reader support)

### Week 11: System Integration
- [ ] DBus integration for system events
- [ ] Systemd service unit for DIAD Agent
- [ ] Auto-update mechanism (check + pull + rebuild)
- [ ] Crash recovery (auto-restart, state recovery)
- [ ] Health monitoring dashboard

### Week 12: Final Polish & Release
- [ ] Comprehensive test suite (>80% coverage)
- [ ] Performance profiling and optimization
- [ ] Documentation (man pages, user guide)
- [ ] Package for AUR (`diad-agent-bin`, `diad-os`)
- [ ] Release v1.0 ISO

### Milestone: v1.0 Release
- Stable, production-ready DIAD OS
- Multi-provider with intelligent routing
- Polished TUI with themes and animations
- Published on AUR and GitHub Releases

---

## Future (Post v1.0)

### On-Device AI
- [ ] Integrate local LLMs via llama.cpp or Ollama
- [ ] Offline mode with local models
- [ ] Hybrid cloud/local orchestration

### Voice Interface
- [ ] Speech-to-text input (Whisper)
- [ ] Text-to-speech output
- [ ] Voice-controlled system navigation

### Plugin System
- [ ] WASM-based plugin runtime
- [ ] Community tool marketplace
- [ ] Custom provider plugins

### Advanced Automation
- [ ] Scheduled tasks and cron integration
- [ ] Event-driven automation (file watchers, system events)
- [ ] Workflow builder (chain intents together)

### Cross-Platform
- [ ] macOS support (via Homebrew)
- [ ] NixOS module
- [ ] Docker container deployment
