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

## Boot Chain

The installed system boots straight into the agent desktop with no login
prompt and no terminal required:

```mermaid
flowchart LR
    A[GRUB] --> B[systemd]
    B --> C[getty@tty1 autologin]
    C --> D[Sway compositor]
    D --> E[diad-supervisor.sh]
    E --> F[diad-hardware-setup.sh]
    E --> G[diad-network-setup.sh]
    E --> H[diad-power-setup.sh]
    E --> I[diad-first-run.sh / diad-splash.sh]
    E --> J[diad-launch.sh]
    J --> K[diad-agent daemon :7860]
    J --> L[Brave app window]
    K --> M[React GUI served by daemon]
    K --> N[TUI client via WebSocket]
```

1. **GRUB → systemd**: normal Arch boot (linux-lts kernel).
2. **Autologin**: `getty@tty1.service.d/autologin.conf` drops straight into the
   `diad` user session.
3. **Sway**: the compositor starts and `exec`s `diad-supervisor.sh`
   (`os/archiso/airootfs/etc/sway/config`).
4. **Supervisor** (`usr/local/bin/diad-supervisor.sh`): runs the marker-gated
   setup scripts (hardware/network/power) *before* the wizard, then calls
   `diad-launch.sh`. No `set -e` — a failing setup step never blocks the
   desktop; every step is logged to `~/.diad/logs/boot.log`.
5. **Launch** (`usr/local/bin/diad-launch.sh`): ensures the `diad-agent` daemon
   is running (it binds `127.0.0.1:7860` and serves the embedded GUI), opens
   the Brave app window on the agent workspace, and Brave on workspace 2.
6. **Firewall**: `diad-firewall.service` (nftables) is enabled at install and
   drops inbound traffic except loopback.

## WebSocket Protocol

The daemon exposes a versioned WebSocket JSON API at `ws://127.0.0.1:7860/ws`
that both the React GUI and the ratatui TUI consume. Every message carries
`"v": 1` (protocol version) and a `"type"` tag; long-running requests carry an
`id` echoed by the daemon so clients can match replies.

**Client → daemon** (`ClientMsg`):

| Type                | Purpose                                        |
|---------------------|------------------------------------------------|
| `chat.send`         | Send chat text (routed to agent loop / offline router) |
| `tool.approve` / `tool.reject` | Approve or reject a pending tool call |
| `edit.approve` / `edit.reject` | Approve or reject a proposed file edit |
| `file.read` / `file.write` / `file.list` | Sandboxed file operations |
| `term.input` / `term.resize` | Daemon-spawned PTY shell I/O |
| `provider.switch` / `model.switch` | Change active provider/model |
| `provider.connect`  | Store and validate an API key |
| `agent.delegate`    | Delegate a task to an external AI CLI |
| `config.set`        | Set a config value |
| `providers.list` / `stats.get` / `models.list` / `agents.list` | Snapshots |
| `secret.submit` / `secret.cancel` | Respond to masked-secret prompts |
| `spawn_tui`         | Open the TUI in a foot terminal (GUI hotkey) |

**Daemon → client** (`ServerMsg`): `token` (streaming deltas), `tool_use`,
`tool_result`, `edit_diff`, `secret.request`, `system`, `error`, `agents`,
`connected` plus stream-end/result messages echoing request `id`s.

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
