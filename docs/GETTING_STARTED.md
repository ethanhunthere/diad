# Getting Started with DIAD

DIAD (Intent-Driven Operating System) is a minimal Arch-based OS that replaces
the traditional desktop workflow with two surfaces: a **Brave browser** and the
**DIAD AGENT** (GUI IDE + TUI) backed by one daemon.

This guide covers three scenarios: building the OS ISO, running the agent on an
existing Linux machine, and the day-to-day developer loop.

---

## 1. Requirements

| Component   | Requirement                                    |
|-------------|------------------------------------------------|
| Rust        | 1.97+ (edition 2021)                           |
| Node.js     | 20+ (frontend build)                           |
| Linux       | Any distro for agent dev; Arch to build the ISO |
| Keyring     | libsecret (gnome-keyring) for API-key storage   |
| Network     | Required only for the LLM API calls themselves  |

---

## 2. Build the agent

```bash
git clone <repo> diad
cd diad/agent

# Debug build (fast iteration)
cargo build --all-targets          # 0 errors, 0 warnings expected

# Release build with the GUI embedded
cd ../frontend
npm install
npm run build                     # writes ../frontend/dist
cd ../agent
cargo build --release             # embeds dist via rust-embed
```

The release binary is `agent/target/release/diad-agent`.

---

## 3. Run the agent

```bash
# 1. Configure providers (stores API keys in the OS keyring)
diad-agent --configure

# 2. Start the daemon + GUI (default)
diad-agent

# 3. Or run the keyboard-first TUI client against the daemon
diad-agent --tui
```

On a DIAD-installed system the daemon and GUI start automatically from Sway via
`diad-supervisor.sh` (see `docs/ARCHITECTURE.md` → Boot Chain).

### First steps

1. Press `F6` (or type `/model`) to pick a provider, then enter your API key.
2. Try a chat message: "what files are in this directory?" — the agent answers
   with a tool call (`list_dir`) that you approve.
3. Type `/model` again to switch models without leaving the chat.

---

## 4. Build the OS ISO

```bash
make iso            # runs `make gui` (frontend build + release agent) first
# output: out/diad-*.iso
```

Flashing to a USB stick:

```bash
# after `make iso-clean` (removes root-owned work/out dirs)
sudo dd if=out/diad-*.iso of=/dev/sdX bs=4M status=progress
```

---

## 5. Configuration

Configuration is layered:

| File                          | Scope  | Example keys                          |
|-------------------------------|--------|---------------------------------------|
| `/etc/diad/diad.conf`         | System | `default_provider`, `theme`           |
| `~/.config/diad/config.toml`  | User   | overrides system values               |

Validate and view the effective config:

```bash
diad-agent --configure
```

---

## 6. Running tests

```bash
cd agent
cargo test          # 282 unit + 29 integration tests
cargo clippy --all-targets -- -D warnings   # must be clean
cargo fmt --check   # must be clean
```

See `docs/TESTING.md` for the full subsystem-by-subsystem manual test guide.
