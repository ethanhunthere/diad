# DIAD Architecture

## Overview

DIAD (Intent-Driven Operating System) presents exactly two user surfaces:

```
┌──────────────────────────────────────────────────────────────┐
│                      DIAD OPERATING SYSTEM                    │
│                                                              │
│  ┌─────────────────────┐    ┌─────────────────────────────┐  │
│  │    Brave Browser     │    │      DIAD Agent (TUI)        │  │
│  │  (Workspace 2)       │    │  (Workspace 1)               │  │
│  │                     │    │                             │  │
│  │  • Web browsing     │    │  • Natural language chat    │  │
│  │  • OAuth flows      │    │  • File management          │  │
│  │  • Documentation    │    │  • Code editing             │  │
│  │  • Web apps         │    │  • System administration    │  │
│  └─────────────────────┘    └─────────────┬───────────────┘  │
│                                           │                   │
│                              ┌────────────▼───────────────┐  │
│                              │      DIAD Agent Core        │  │
│                              │                             │  │
│                              │  ┌───────────────────────┐  │  │
│                              │  │    Intent Router       │  │  │
│                              │  │  classify_input()     │  │  │
│                              │  │  route_to_provider()  │  │  │
│                              │  └─────────┬─────────────┘  │  │
│                              │            │                 │  │
│                              │  ┌─────────▼─────────────┐  │  │
│                              │  │   Provider Adapters    │  │  │
│                              │  │                        │  │  │
│                              │  │  Anthropic  OpenAI     │  │  │
│                              │  │  Gemini     Qwen       │  │  │
│                              │  └─────────┬─────────────┘  │  │
│                              │            │                 │  │
│                              │  ┌─────────▼─────────────┐  │  │
│                              │  │    Tool Registry       │  │  │
│                              │  │                        │  │  │
│                              │  │  Filesystem  Process   │  │  │
│                              │  │  Network     System    │  │  │
│                              │  └───────────────────────┘  │  │
│                              └─────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Component Diagram

### Layer 1: User Surfaces

| Surface | Technology | Purpose |
|---------|-----------|---------|
| Brave Browser | Chromium-based | Web browsing, OAuth authentication, documentation |
| DIAD Agent | Rust TUI (ratatui + crossterm) | Intent input, file management, code editing, system admin |

### Layer 2: TUI Framework

The Terminal UI is built with:

- **ratatui**: Immediate-mode terminal rendering library
- **crossterm**: Cross-platform terminal manipulation (raw mode, events, styling)

Components:
- `App` — holds all application state (input buffer, messages, mode, provider selection)
- `ui::render()` — draws the complete frame: top bar, main content, bottom input bar
- `EventLoop` — polls crossterm events and sends them through a tokio mpsc channel

### Layer 3: Orchestrator

The orchestrator bridges user intent and provider execution:

- **IntentRouter**: Classifies user input as `Chat`, `ToolCall`, `EditCommand`, or `SystemQuery`
- **SessionManager**: Persists conversation sessions as JSON in `~/.diad/sessions/`

### Layer 4: Provider Adapters

Each LLM provider has a dedicated adapter implementing the `LlmProvider` trait:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamEvent>> + Send>>>;

    fn provider_name(&self) -> String;
    fn requires_auth(&self) -> bool;
}
```

| Provider | API | Key Features |
|----------|-----|-------------|
| AnthropicProvider | Anthropic Messages API | SSE streaming, tool_use blocks, Claude models |
| GeminiProvider | Google Generative Language API | SSE streaming, function calling |
| OpenAiProvider | OpenAI Chat Completions API | SSE streaming, function calling, GPT models |
| QwenProvider | Alibaba DashScope API | SSE streaming, tool calls, Qwen models |

All providers use `reqwest` with byte streaming and parse Server-Sent Events.

### Layer 5: Tool System

Tools are sandboxed Rust functions registered in a `ToolRegistry`:

| Category | Tools |
|----------|-------|
| **Filesystem** | `read_file`, `write_file`, `list_dir`, `search_files`, `chmod_file` |
| **Process** | `spawn_process`, `kill_process`, `list_processes`, `get_process_logs` |
| **Network** | `http_get`, `http_post`, `ping_host`, `get_network_status` |
| **System** | `install_package`, `remove_package`, `manage_service`, `get_system_info`, `read_config`, `write_config` |

### Layer 6: Authentication & Security

- **Vault**: Uses the OS keyring (`libsecret`/GNOME Keyring) for secure token storage
- **OAuthFlow**: Handles OAuth 2.0 authorization code flow with PKCE, opens Brave for consent

### Layer 7: Configuration

- **DiadConfig**: Loads from `/etc/diad/diad.conf` (system) and `~/.config/diad/config.toml` (user)
- Settings: default provider, allowed paths, log level, theme, keybindings, streaming preferences

## Data Flow

### Chat Input Flow

```
User types intent
       │
       ▼
App::handle_key_event() — buffers input
       │
       ▼
App::submit_input() — on Enter
       │
       ▼
IntentRouter::classify_input() — determines intent type
       │
       ▼
ProviderFactory::create() — instantiates provider adapter
       │
       ▼
LlmProvider::stream_chat() — sends HTTP POST with SSE streaming
       │
       ▼
Event::ProviderResponse / Event::ToolResult — updates App state
       │
       ▼
ui::render() — redraws TUI with new messages
```

### Tool Call Flow

```
LLM responds with tool_use block
       │
       ▼
StreamEvent::ToolUseStart / ToolInputDelta / ToolUseEnd
       │
       ▼
ToolRegistry::execute_tool() — calls registered handler
       │
       ▼
Sandbox checks (path validation, destructive confirmation)
       │
       ▼
ToolResult returned → sent back to LLM as tool result message
       │
       ▼
LLM processes result → streams final text response
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| **Base OS** | Arch Linux (minimal) |
| **Display Server** | Wayland |
| **Window Manager** | Sway (compositor) |
| **Terminal** | foot |
| **Browser** | Brave |
| **Agent Language** | Rust (Edition 2021) |
| **Async Runtime** | tokio |
| **TUI** | ratatui + crossterm |
| **HTTP** | reqwest (streaming) |
| **Serialization** | serde + serde_json + toml |
| **Secrets** | keyring (libsecret) |
| **Logging** | tracing + tracing-subscriber |
| **CLI** | clap |
| **Errors** | anyhow + thiserror |
| **Testing** | cargo test + tokio-test |

## File Locations

| Path | Purpose |
|------|---------|
| `/usr/local/bin/diad-agent` | Agent binary |
| `/usr/local/bin/diad-launch.sh` | Launch script |
| `/etc/diad/diad.conf` | System config |
| `/etc/sway/config` | Sway WM config |
| `~/.config/diad/config.toml` | User config |
| `~/.diad/sessions/` | Session persistence |
| `~/.diad/logs/` | Runtime logs |
