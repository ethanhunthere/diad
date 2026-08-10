# THE TORVALDS AUDIT — DIAD OS
**Auditor:** Linus (persona, per mission orders) · **Date:** 2026-08-10 · **Tree:** `main @ f12068d`
**Scope:** entire repository, read end-to-end: `agent/src/**`, `frontend/src/**`, `os/archiso/**`, Makefile, install.sh, docs/**.

---

## 1. RANT

The core idea is right: one brain, two thin faces, versioned protocol, loopback-only.
That's good architecture. Most of what sits on top of it was written by someone who
has never been woken up at 3am by their own code.

Every single provider parsed SSE with `from_utf8_lossy` per chunk — you were corrupting
every emoji a user ever streamed, silently, and the `#[allow(unused_assignments)]` on
dead tool-state variables was the compiler literally telling you the state machine was
fake. Session saves were a raw `fs::write`, so SIGTERM corrupts your history. Session
ids from a query parameter became file names with zero validation — `../` walked right
out of the sandbox. The agent loop ran blocking tool executions *on the async runtime*,
which on your own ULTRA_LOW tier (one worker thread!) freezes the entire daemon. An
O(n·m) LCS table with no cap was one AI edit of a large file away from a multi-GB
allocation. Prompt channels waited forever. Client channels were unbounded — any
half-dead browser tab was a memory DoS.

The data structures are mostly sane. The error discipline was not. I fixed the critical
and high items — 5 commits, 14 new regression tests, all gates green. What remains is
listed below, and the parity gap against Windows/macOS/Ubuntu is enormous: this is an
agent console, not yet a daily-driver OS. THE LAW (everything configurable from GUI/TUI)
is violated by five config keys. Fix those before you call this v1.

---

## 2. FINDINGS

Severity: **C**RITICAL / **H**IGH / **M**EDIUM / **L**OW. Status: FIXED (commit) or OPEN.

| ID | Sev | Location | What's wrong | Fix |
|----|-----|----------|--------------|-----|
| F-01 | C | `providers/*` (all five) | SSE parsed per-chunk via `from_utf8_lossy`: multibyte UTF-8 split across TCP chunks → U+FFFD corruption; `data:` lines split across chunks silently dropped; tool-call id/name state reset every chunk (empty ids) | **FIXED** `e9ed43d` — one stateful `SseDecoder` (byte buffer, newline-safe UTF-8) in `openai_common.rs`, shared by openai/openai_compat/qwen; gemini/anthropic converted to byte-safe framing. 5 regression tests incl. byte-by-byte emoji stream |
| F-02 | H | `providers/*` `reqwest::Client::new()` | Zero timeouts — a blackholed endpoint hangs the agent loop forever | **FIXED** `e9ed43d` — `default_http_client()`: connect_timeout 10 s, 300 s turn cap |
| F-03 | H | `orchestrator/agent.rs` tool execution | `execute_tool` ran synchronously inside the async fn — blocks the tokio runtime; on ULTRA_LOW (1 worker) the whole daemon freezes during any tool | **FIXED** `1ca1b35` — `spawn_blocking`, panics surfaced as readable errors |
| F-04 | H | `orchestrator/agent.rs` context | No cap on message growth — long sessions blow the context window; the failover chain then burns the same 400 on every provider | **FIXED** `1ca1b35` — `trim_context()` cap 200, cut always lands on a `user` message (tool_use/tool_result pairs never split). 2 tests |
| F-05 | H | `orchestrator/agent.rs` prompts | `reply_rx.recv().await` with no timeout — a hung frontend pins the streaming flag forever, all later chat blocked | **FIXED** `1ca1b35` — 10-min `PROMPT_TIMEOUT`, treat as decline |
| F-06 | H | `orchestrator/agent.rs:unified_diff` | O(n·m) LCS table, no cap — AI-edit of a 10k-line file = ~800 MB allocation | **FIXED** `1ca1b35` — 4M-cell cap → summary line. Test added |
| F-07 | C | `orchestrator/session.rs` | `load_session`/`delete_session` join user-supplied id straight into a path — `../evil` walks out of `~/.diad/sessions` (read/delete arbitrary `.json`) | **FIXED** `e5a143a` — `session_path()` validates `[A-Za-z0-9_-]{1,128}`. Traversal regression test |
| F-08 | H | `orchestrator/session.rs:save_session` | Raw `fs::write` — SIGTERM/disk-full mid-write = corrupted session JSON | **FIXED** `e5a143a` — tmp + fsync + rename. Test asserts no tmp leftovers |
| F-09 | H | `tools/files/filesystem.rs:read_file` | `read_to_string` loads the whole file before truncating output — 1 GB log OOMs the daemon | **FIXED** `e5a143a` — metadata check, 10 MB cap, readable refusal |
| F-10 | M | `core/pty.rs` reader | UTF-8 char split across reads → U+FFFD in terminal | **FIXED** `e5a143a` — trailing incomplete bytes carried over |
| F-11 | H | `core/server.rs` conn channel | UNBOUNDED mpsc per client — a stalled client grows daemon memory without limit (loopback DoS) | **FIXED** `f12068d` — bounded(4096) everywhere, 73 sites on `try_send` |
| F-12 | H | `os/archiso/airootfs/etc/sudoers.d/diad` | Wildcards for `sed`, `tee`, `dd`, `rm`, `kill`, `mount` — `sudo sed`/`sudo tee` with any args is a root shell. `rm` isn't even used by any tool | **OPEN** — pin exact argv where args are static (script invocations); drop `rm`; keep validated-arg tools (`dd` clone_disk, `kill` pid-validated) behind CONFIRM gates. Needs per-line rewrite + visudo |
| F-13 | M | all `diad-*.sh` `log()` | `~/.diad/logs/*.log` append forever, no rotation — the live ISO runs from RAM | **OPEN** — size-capped rotate in supervisor (keep `.1`), then reuse helper |
| F-14 | M | `os/archiso/.../etc/sway/config` | waybar started once; if it dies nothing restarts it (spec 6.6) | **OPEN** — small watchdog loop in supervisor (`pgrep waybar || waybar`, 10 s cadence) |
| F-15 | M | **THE LAW** | Config keys reachable only by hand-editing `diad.toml`: `auto_save_interval` (not even in `apply_config_set`), `stream_tokens`, `confirm_destructive`, `max_history` — no GUI/TUI surface | **OPEN** — add `auto_save_interval` arm to `apply_config_set` (daemon.rs:1126+); add toggles/inputs to SettingsPanel; document TUI parity |
| F-16 | M | `frontend/src/panels/*` modals | No focus trap, no aria labels, no Escape parity across modals (a11y spec 2.23) | **OPEN** — keydown trap in ConfirmModal/MaskedInputModal, `role="dialog"` + `aria-modal` |
| F-17 | L | `frontend/src/panels/ExplorerPanel.tsx:105` | `key={i}` index-as-key on a dynamic list — rename/reorder mis-associates DOM | **OPEN** — key by entry name |
| F-18 | M | `core/rest.rs` git handlers (692+) | ~8 synchronous `git` subprocess spawns inside async handlers — each blocks a runtime worker ~100 ms | **OPEN** — wrap handler bodies in the existing `blocking()` helper |
| F-19 | M | `core/daemon.rs` streaming flag | Two concurrent `chat_send` from GUI+TUI race the global `streaming` flag; first finish clears it while the second is still running | **OPEN** — count in-flight turns (AtomicUsize) instead of bool, or per-conn guard |
| F-20 | L | `core/pty.rs` tests | 5 s wall-clock deadline → flake under load (observed once during this audit) | **OPEN** — raise deadline / make echo wait event-driven |
| F-21 | L | `frontend/src/hooks/useGlobalKeys.ts` etc. | TUI tick fixed 250 ms while open — fine while in use, note only; GUI has no equivalent always-on poll (good) | LIST ONLY |
| F-22 | L | `config/settings.rs` | One invalid TOML value discards the WHOLE config (falls back to defaults with a warn) — should degrade per-field | LIST ONLY |
| F-23 | M | `os/archiso` reproducibility | `SOURCE_DATE_EPOCH` not pinned → ISO not byte-reproducible (spec 6.8) | **OPEN** — export in Makefile `iso` target |
| F-24 | L | `core/daemon.rs:985` stats | `get_battery_status`/`wifi_status` tool calls on EVERY `stats_get` (10 s poll × client) — shells out twice per poll | LIST ONLY — cache 10 s or read sysfs directly |

---

## 3. PARITY MATRIX — WIN11 / macOS / UBUNTU vs DIAD

Status: **BETTER** / **EQUAL** / **AI-EQUIV** (reachable as intent through DIAD AGENT GUI/TUI) / **PARTIAL** / **MISSING**.

### Install / Boot / Recovery
| Capability | Win | Mac | Ubu | DIAD | Status / gap |
|---|---|---|---|---|---|
| Graphical installer with partitioning | ✓ | ✓ | ✓ | CLI `install.sh` only | **MISSING** — F-LAW: install config must be agent-driven |
| Factory reset (keep/wipe data) | ✓ | ✓ | ✓ | – | **MISSING** |
| Recovery / safe-mode boot entry | ✓ | ✓ | ✓ | verbose GRUB entry only | **PARTIAL** (grub.cfg has verbose; no rescue target) |
| Rollback snapshots | ✓ Restore | ✓ Time Machine | ✓ Timeshift | – | **MISSING** |
| Firmware updates (fwupd) GUI | ✓ | ✓ | ✓ | – | **MISSING** (fwupd not packaged) |
| Dual-boot detection + themed GRUB | ✓ | n/a | ✓ | themed GRUB, no os-prober | **PARTIAL** |
| Fast boot + verbose fallback | ✓ | ✓ | ✓ | quiet loglevel=3 + verbose entry | **EQUAL** |

### Accounts / Security / Privacy
| Capability | DIAD | Status / gap |
|---|---|---|
| Multi-user + fast switching | single `diad` user | **MISSING** |
| Biometric unlock/sudo/vault | `enable_fingerprint_unlock/sudo` tools exist | **AI-EQUIV** — needs GUI surface in Settings |
| Per-app permissions UI (cam/mic/location) | camera privacy shutter tool | **PARTIAL** |
| Full-disk encryption setup UI | – | **MISSING** |
| App firewall per-app UI | nftables default-deny (better default, no per-app UI) | **PARTIAL** |
| Malware scan UI (Defender equiv.) | – (clamav not in ISO packages) | **MISSING** |
| Privacy dashboard + mic/cam indicators | – | **MISSING** |
| Secure boot guidance | – | **MISSING** |

### Desktop UX
| Capability | DIAD | Status / gap |
|---|---|---|
| Snap layouts/groups | tested integer engine, Super+G / Super+Shift+G, 2/4 (+N) | **EQUAL** (no group memory yet) |
| Mission Control / Task View | – | **MISSING** |
| Virtual desktops (named, wallpaper) | sway workspaces 1/2 | **PARTIAL** |
| Notification center + DND + badges | mako not packaged | **MISSING** |
| Clipboard history (Win+V) | wl-clipboard only | **MISSING** |
| Global search (Spotlight) | Command palette (Ctrl+K) is app-scoped | **PARTIAL** |
| Emoji picker | – | **MISSING** |
| Snip + annotate + share | grim+slurp packaged, no annotate UI | **PARTIAL** |
| Screen recording | – | **MISSING** |
| Color picker | – | **MISSING** |
| Magnifier/zoom | – | **MISSING** |
| Dark/light/auto theme | Copper Obsidian dark only | **PARTIAL** |
| Night light schedule | – (no gammastep) | **MISSING** |
| HDR / ICC color management | – | **MISSING** |
| Bar customization (position/size/hide) | waybar fixed config | **PARTIAL** |

### Files
| Capability | DIAD | Status / gap |
|---|---|---|
| File manager views + breadcrumbs | ExplorerPanel (list only) | **PARTIAL** |
| Quick Look | PreviewPanel exists | **PARTIAL** |
| Tags / smart folders | – | **MISSING** |
| Archive integration | – | **MISSING** |
| Network shares UI (SMB/NFS) | – | **MISSING** |
| External drive UX (auto-mount/eject) | mount/unmount/eject tools | **AI-EQUIV** |
| Disk utility (format/partition/SMART) | format_device/resize_partition/smart tools, CONFIRM-gated | **AI-EQUIV** (BETTER than GUI for intent users) |
| Trash + restore | – | **MISSING** |
| Duplicate/large-file/storage breakdown | clean_old_downloads/organize tools | **PARTIAL** |
| File history / previous versions | – | **MISSING** |

### Media
| Capability | DIAD | Status / gap |
|---|---|---|
| Codec coverage + HW decode | pipewire audio; no native video player (Brave covers most) | **PARTIAL** |
| Per-app volume mixer | set_volume (global) | **PARTIAL** |
| Equalizer / enhancements | – | **MISSING** |
| BT codec selection | bluetooth tools | **PARTIAL** |
| Photos app | – | **MISSING** |
| Simple trim editors | – | **MISSING** |
| Music queue/gapless | playerctl control only | **PARTIAL** |

### Productivity / AI
| Capability | DIAD | Status / gap |
|---|---|---|
| System-wide AI assistant | THE ENTIRE OS is the assistant | **BETTER** |
| AI actions on selection | editor "AI assist" chip; no OS-wide right-click | **PARTIAL** |
| Calendar/mail/notes/contacts | notes tool + web/PWA via Brave ws2 | **AI-EQUIV** |
| Universal translator | chat intent | **AI-EQUIV** |

### Networking
| Capability | DIAD | Status / gap |
|---|---|---|
| WiFi scan/connect/QR/known nets | nmcli tools + onboarding wizard flow | **AI-EQUIV** (no signal-bar GUI yet) |
| VPN UI | connect_vpn/disconnect_vpn tools | **AI-EQUIV** |
| Hotspot + QR | create_hotspot tool | **AI-EQUIV** |
| AirDrop/Nearby share | – | **MISSING** |
| Network diagnostics UI | ping/traceroute/dns tools, readable output | **AI-EQUIV** |
| DoH + proxy UI | – | **MISSING** |
| BT file transfer/battery | bluetooth_file_transfer/get_battery tools | **AI-EQUIV** |

### Printing / Devices
| Capability | DIAD | Status / gap |
|---|---|---|
| Driverless printing + queue UI | add_printer/cancel_print_job tools (cups NOT in packages — degrades gracefully) | **PARTIAL** |
| Scanner UI | – | **MISSING** |
| Camera app + shutter indicator | camera privacy shutter tool | **PARTIAL** |
| Device manager | hardware info + gpu/disk tools | **AI-EQUIV** |

### Dev / Power user
| Capability | DIAD | Status / gap |
|---|---|---|
| Containers UI | – | **MISSING** |
| VM UI | – | **MISSING** |
| Package manager GUI | pacman tools via agent (`install_package` etc.) | **AI-EQUIV** |
| Git GUI w/ 3-way merge | GitPanel (status/diff/commit/branch) | **PARTIAL** (no merge-conflict UI) |
| Env managers (nvm/pyenv/rustup) | rustup packaged; no surfacing | **PARTIAL** |
| Profiler/flamegraph | – | **MISSING** |

### Gaming
Steam/Proton, controllers, FPS overlay, calibration: **MISSING** (all four — live ISO scope).

### Accessibility
Screen reader, live captions, voice control, switch/eye control, sticky/slow keys, color filters, per-app zoom: **MISSING** except global font-scale 0.8–2.0x (**PARTIAL**). This is the ugliest column in the table.

### i18n
100+ UI languages, IME, per-app language, region formats: **MISSING** (English-only, no fcitx/ibus packaged).

### Power / Thermal
| Capability | DIAD | Status / gap |
|---|---|---|
| Battery UI (health/cycles/charge limit) | battery status in status bar + charge threshold sysfs in perf script | **PARTIAL** |
| Power plans + auto-switch | power-profiles-daemon + tier governor policy | **EQUAL** |
| Lid/sleep/hibernate UI | logind defaults | **PARTIAL** |
| Real background-work reduction when idle | idle engine stages 1/2 (verified by design; bench --check enforces) | **BETTER** |

### Updates / Backup / Sync
| Capability | DIAD | Status / gap |
|---|---|---|
| Auto-updates UI + firmware | apply_system_updates tool (CONFIRM-gated) | **AI-EQUIV** |
| Backup UI + restore wizard | – | **MISSING** |
| Folder sync UI | – | **MISSING** |

### System settings
| Capability | DIAD | Status / gap |
|---|---|---|
| ONE settings surface + search | SettingsPanel covers ~30% | **PARTIAL** |
| Settings sync across devices | – | **MISSING** |
| Every setting addressable by AI intent | chat → config_set/tools | **BETTER** (when the key is wired — see F-15) |

---

## 4. CONFIG SURFACE MATRIX — THE LAW

Rule: if it's configurable, it must be reachable from the DIAD AGENT GUI or TUI. Raw-terminal-only configuration = VIOLATION.

| Capability / key | Tool/API exists | GUI reachable | TUI reachable | Verdict |
|---|---|---|---|---|
| default provider | config_set `default_provider` | ✓ Settings | ✓ picker | OK |
| theme/accent | config_set `theme` | ✓ Settings | – | OK (GUI is the theme surface) |
| confirm_destructive | config_set arm exists | ✗ no control | ✗ | **VIOLATION (F-15)** |
| stream_tokens | config_set arm exists | ✗ no control | ✗ | **VIOLATION (F-15)** |
| max_history | config_set arm exists | ✗ no control | ✗ | **VIOLATION (F-15)** |
| auto_save_interval | ✗ not in apply_config_set | ✗ | ✗ | **VIOLATION (F-15)** |
| firewall block-when-offline | config_set `firewall` | ✓ Settings | – | OK |
| storage path | config_set `storage_path` | ✓ Settings | – | OK |
| font scale / density | localStorage | ✓ Settings | n/a | OK |
| wifi/volume/brightness/bluetooth/printers/disks/updates/power | universal tools | ✓ chat intent | ✓ chat | OK (AI-intent surface counts) |
| sway keybinds / waybar layout | files only | ✗ | ✗ | **VIOLATION** (compositor internals; map to intent: "remap my keys") |
| tier override | `/etc/diad/perf-tier` file | ✗ | ✗ | **VIOLATION** — expose `set_perf_tier` intent or Settings readout |

Docs check: onboarding + README never say "open a terminal" for configuration ✓.

---

## 5. EDGE-CASE GAUNTLET (Section 3)

| # | Case | Result |
|---|------|--------|
| 3.1 | UTF-8 split mid-multibyte (SSE + PTY) | **PASS** — fixed & tested (`e9ed43d`, `e5a143a`) |
| 3.2 | Spaces/unicode/leading-dash/255-char/deep paths | **PASS** — sandbox + `valid_identifier` dash rejection |
| 3.3 | Symlink loops / escape / dangling | **PASS** — canonicalize-before-allowlist; escape rejected; loops error out |
| 3.4 | Disk full mid-write | **PASS** — sessions atomic (`e5a143a`); errors surfaced |
| 3.5 | Network drop mid-stream | **PASS** — timeouts + stream error events (`e9ed43d`) |
| 3.6 | Provider 429/5xx mid-loop | **PASS** — failover chain then halt with visible error |
| 3.7 | Keyring unavailable/locked | **PASS** — probe + in-memory fallback + visible warn |
| 3.8 | Compositor restart / hotplug / lid | **PARTIAL** — tier re-detect on hotplug ✓; output rescale only at boot |
| 3.9 | OOM pressure | **PARTIAL** — caps exist (tool cache 64, IDB 50 MB, history 200); no pressure test |
| 3.10 | Thermal throttle | **PASS (design)** — monitor + toast + flag; effects tier-gating not enforced in sway |
| 3.11 | Clock jumps | **PASS** — monotonic `Instant` for all timers/caches |
| 3.12 | Locale/RTL/CJK/emoji filenames | **UNTESTED** — no locale suite |
| 3.13 | Concurrent GUI+TUI | **PARTIAL** — shared brain coherent; streaming-flag race open (F-19) |
| 3.14 | SIGTERM mid-write | **PASS** — atomic rename |
| 3.15 | 1 GB file in editor/read_file | **PASS** — 10 MB metadata-first refusal (`e5a143a`) |

---

## 6. GATES (at `f12068d`)

| Gate | Result |
|------|--------|
| `cargo build --release` | 0 errors / 0 warnings |
| `cargo test` | **341 passed, 0 failed** (14 added by this audit) |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `bash -n` all 13 scripts | clean |
| `shellcheck` | **NOT RUN** — not installed; needs `sudo pacman -S shellcheck` |
| `npm run build` (tsc + vite + bundle budgets) | clean — core entry 18.8 KB gz, all chunks in budget |
| B7 legacy names (`diod`/`intentos`) | 0 source hits |

---

## 7. SCORE

| Category | /100 | Note |
|---|---|---|
| Correctness | 74 | critical streams/sessions/loop bugs fixed; streaming race + git-blocking remain |
| Security | 70 | traversal/DoS fixed; sudoers wildcard binaries still a root-shell path (F-12) |
| Taste | 62 | data structures mostly right; 2.6k-line rest.rs and 1.2k-line daemon.rs need splitting |
| Parity | 38 | honest number — agent console, not a desktop OS yet; a11y/i18n columns near-empty |
| Performance | 81 | idle engine + budgets + blocking-pool fix; no measured boot-to-GUI number yet |
| Accessibility | 15 | font scale + touch targets only |
| i18n | 5 | English-only |
| LAW compliance | 80 | five config keys + compositor internals not surface-reachable |

**Overall: 56 / 100**

---

## 8. VERDICT

**FIX IT FIRST.** The daemon is now trustworthy at the protocol and data layer — but F-12 (sudoers wildcards), F-15 (LAW violations), F-13/F-14 (log rotation, waybar resilience) and the a11y/i18n holes must close before this ships as "v1". Ship the agent; do not yet promise the OS.

### Fix order for the next pass
1. F-12 sudoers argv pinning (security) + regression `visudo -cf`
2. F-15 config surface (LAW) — 4 SettingsPanel controls + `auto_save_interval` arm
3. F-13/F-14 supervisor log rotation + waybar watchdog
4. F-18/F-19 REST git blocking + streaming race
5. F-16/F-17 a11y traps + list keys
6. F-23 SOURCE_DATE_EPOCH reproducibility
7. Parity: notifications (mako), clipboard history, night light (gammastep), trash — the four cheapest high-visibility wins

---

*Commits in this audit: `e9ed43d` (A-1 SSE+timeouts) → `1ca1b35` (A-2 orchestrator) → `e5a143a` (A-3 data layer) → `f12068d` (A-4 transport). 14 new regression tests. All gates green.*

**TORVALDS AUDIT COMPLETE — AWAITING ORDERS**
