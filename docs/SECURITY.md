# DIAD Security Model

## Threat Model

### Assets to Protect

| Asset | Sensitivity | Storage |
|-------|------------|---------|
| LLM API Keys | High | OS Keyring (libsecret) |
| OAuth Tokens | High | OS Keyring (libsecret) |
| Session History | Medium | `~/.diad/sessions/` (JSON, permissions 600) |
| User Files | Variable | Filesystem (sandboxed tool access) |
| System Configuration | High | `/etc/diad/` (root-owned, 644) |

### Threat Actors

| Actor | Capability | Mitigation |
|-------|-----------|------------|
| Remote attacker (network) | Attempt to inject malicious tool calls via LLM responses | Tool parameter validation, path sandboxing |
| Malicious LLM response | LLM generates harmful tool calls | Destructive action confirmation, allowed path restrictions |
| Local unprivileged user | Read session files, config | Unix file permissions (600/700) |
| Compromised browser | Steal OAuth tokens from memory | Token isolation in keyring, never in browser storage |
| Supply chain (crate) | Malicious dependency | Cargo.lock pinning, audit with `cargo audit` |

## Sandboxing Rules

### Filesystem Tools

All filesystem operations are sandboxed to `allowed_paths` defined in configuration:

```toml
allowed_paths = ["~", "/tmp", "/etc/diad"]
```

Rules:
1. **Read**: Only files within allowed paths
2. **Write**: Only files within allowed paths, parent directories auto-created
3. **Delete**: Requires explicit confirmation (`confirm_destructive = true`)
4. **List**: Only directories within allowed paths
5. **Chmod**: Only files within allowed paths

### Process Tools

1. **Spawn**: Commands run as the current user (no privilege escalation)
2. **Kill**: Only processes owned by the current user
3. **List**: Uses `ps aux` — only shows user-visible processes

### Network Tools

1. **HTTP**: Outbound requests only (no listening servers)
2. **Ping**: ICMP echo only
3. **No port scanning**: Tools don't support scanning or probing

### System Tools

1. **Package Management**: Runs `pacman` through `sudo` (requires user in wheel group)
2. **Service Management**: Runs `systemctl` through `sudo`
3. **Config Write**: Only to `~/.config/diad/config.toml`

## Token Storage Policy

API keys and OAuth tokens are stored exclusively in the system keyring:

```
┌─────────────────────────────────────┐
│          GNOME Keyring               │
│  (libsecret / org.freedesktop       │
│   .secrets)                         │
│                                     │
│  Entry: diad-agent-anthropic         │
│  Entry: diad-agent-gemini            │
│  Entry: diad-agent-openai            │
│  Entry: diad-agent-qwen              │
└─────────────────────────────────────┘
```

Rules:
1. **Never** stored in environment variables (`.env` files are ignored)
2. **Never** stored in configuration files
3. **Never** logged (tracing filters mask sensitive values)
4. **Never** included in session files saved to disk
5. Token access is **read-on-demand** — loaded from keyring at provider creation, not cached in memory beyond the provider struct lifetime

### Keyring Entry Format

```
Service:  diad-agent
Username: diad-agent-{provider_name}
Password: {api_key_or_token}
```

## Destructive Action Confirmation

When `confirm_destructive = true` (default), the agent:

1. Detects the intent as destructive (file deletion, package removal, service disable, etc.)
2. Displays a confirmation prompt in the TUI: `⚠️  Destructive action: [description]. Proceed? [y/N]`
3. Requires explicit `y` or `yes` input before executing
4. Logs the action to `~/.diad/logs/audit.log` with timestamp and action description

Destructive actions include:
- `write_file` (overwriting existing files)
- `chmod_file` (changing permissions)
- `remove_package` (uninstalling packages)
- `manage_service` with "stop" or "disable"
- `kill_process`

## Audit Logging

All tool executions are logged via `tracing`:

```json
{
  "timestamp": "2026-08-08T12:34:56Z",
  "level": "INFO",
  "target": "diad_agent::tools",
  "tool": "write_file",
  "path": "/home/diad/test.txt",
  "bytes": 1024,
  "success": true
}
```

Logs are written to:
- `~/.diad/logs/agent.log` (general)
- `~/.diad/logs/audit.log` (security-relevant)
- Systemd journal (if running as a service)

## Network Security

1. **TLS Everywhere**: All LLM provider API calls use HTTPS (reqwest enforces TLS by default)
2. **Certificate Validation**: Default reqwest behavior — validates against system CA store
3. **No MITM Proxies**: The agent does not trust custom CA certificates
4. **OAuth State**: CSRF protection via random `state` parameter in OAuth flows

## Recommendations

1. Run `gnome-keyring-daemon` at session start (auto-launched by Sway config)
2. Set strong login password (protects keyring master key)
3. Keep system updated: `pacman -Syu`
4. Audit dependencies: `cargo audit`
5. Review allowed paths: restrict to minimum necessary
6. Enable disk encryption (LUKS) on the host system
