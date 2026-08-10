# DIAD Codebase Sanitization — Final Report

**Scope**: Full google-grade refactor of `/home/f0id/diados` — dead files,
dead code, unused dependencies, module restructure, test reorganization,
documentation, consistency, git hygiene, and release gates.

**Branch**: `main` · **Baseline**: `e9809cf` (pre-sanitization checkpoint) ·
**Head**: `0d787ca` · **Tag**: `v0.9-pre-gui`

---

## 1. DELETION MANIFEST

Every deletion was committed separately and is recoverable from git history.
No file referenced by `Makefile`, `install.sh`, `profiledef.sh`, sway config,
supervisor scripts, systemd units, or any Rust `mod`/`use` statement was
deleted — anything uncertain was restructured in place instead.

| Phase | Commit | What was removed | Δ lines |
|-------|--------|------------------|---------|
| 1 | `deae549` | Stale duplicate `agent/` + `frontend/` trees, scratch files, build logs, staged ISO binary (gitignored), dead docs | **−8,645** across 35 files |
| 2 | `c5de4e1` | Dead code: unreachable functions, unused helpers, orphaned structs | −94 |
| 3 | `a090e86` | Unused dependencies from `Cargo.toml` (incl. `thiserror`, tokio-test, and others with zero usage) | −295 |
| 4b | `7547bc4` | Monolithic `core/server.rs` 1,232 lines replaced by split `daemon.rs` (brain) + `server.rs` (transport) | −1,232 |
| 5 | `dc1d1b8` | Monolithic `tests/integration_test.rs` (408 lines) replaced by 6 domain test files | −408 |

**Quarantined files**: none — every uncertain item was restructured, never
deleted.

---

## 2. RESTRUCTURE MAP

### Rust — `agent/src/`

```
Before                            After
────────────────────────────────  ─────────────────────────────────────────
daemon/ (monolithic)              core/          daemon.rs (brain) · server.rs (transport)
                                      protocol.rs (versioned WS) · pty.rs · rest.rs ·
                                      debug.rs · notes.rs · usage.rs
tools/mod.rs (1,500+ lines)       tools/         mod.rs (types + testutil)
                                                   files/    files_universal · filesystem
                                                   hardware/ device · display · input · hardware_universal
                                                   network/  connectivity · external
                                                   system/   exec · help · os · power · process ·
                                                             security · update
                                                   registry.rs (59 tools, schema-validated)
system/system.rs  →  system/os.rs      (clippy self_named_module)
hardware/hardware.rs → hardware/device.rs
network/network.rs → network/connectivity.rs
```

### Frontend — `frontend/src/`

```
Before                            After
────────────────────────────────  ─────────────────────────────────────────
store/useStore.ts                 stores/useStore.ts
lib/api.ts                        services/api.ts
index.css                         styles/index.css
lib/ + mixed panels               components/  10 shared primitives (TopBar, ToolCallCard,
                                                EditDiffCard, ChatInput, MaskedInputModal, …)
                                  panels/       11 views (Chat, Editor, Terminal, Explorer,
                                                Agents, Settings, Git, Debug, Preview, Projects, Onboarding)
                                  hooks/        useWebSocket · useGlobalKeys
                                  lib/          types.ts · format.ts
```

### Tests — `agent/tests/`

```
integration_test.rs (408 lines)    tui_app_tests.rs (10) · tools_sandbox_tests.rs (8) ·
                                   tools_system_tests.rs (4) · orchestrator_tests.rs (4) ·
                                   config_tests.rs (2) · providers_tests.rs (1) ·
                                   common/mod.rs (shared helpers)
```

---

## 3. DEPENDENCY MANIFEST

**Rust runtime deps (28)**: tokio (full), axum 0.8, tokio-tungstenite,
reqwest, async-trait, serde, serde_json, config 0.14, toml, clap 4, anyhow,
chrono, uuid, directories 5, futures, futures-util, bytes, glob, shellexpand 3,
urlencoding, regex, ratatui 0.28, crossterm, portable-pty, rust-embed 8,
keyring, tracing, tracing-subscriber.

**Rust dev deps (2)**: tempfile 3, tower 0.5.

**Frontend**: React 18.3.1, zustand 5.0.2, @uiw/react-codemirror, @xterm/xterm
5.5, tailwindcss 3.4, vite 6, TS 5.7. Zero `any` types, zero unused imports
(build gate enforces).

Every dependency is pinned by `Cargo.lock` / `package-lock.json`. Removed in
Phase 3: `thiserror`, `tokio-test`, `tower-http` (server), and others with no
usage in the codebase.

---

## 4. DOCUMENTATION COVERAGE

| Document | Coverage |
|----------|----------|
| `README.md` | Overview, architecture (mermaid), provider matrix, philosophy, boot, quick start, keybindings, project structure |
| `docs/ARCHITECTURE.md` | Component diagram, **boot chain (GRUB→autologin→Sway→supervisor→launch→daemon)**, **WebSocket protocol (ClientMsg/ServerMsg tables)**, data flows, tech stack |
| `docs/SECURITY.md` | Threat model, sandboxing rules, keyring policy |
| `docs/TESTING.md` | Per-subsystem manual + automated test map, accurate counts (311) |
| `docs/GETTING_STARTED.md` | New — build/run/ISO/config/test walkthrough |
| `docs/CONTRIBUTING.md` | New — layout, tool/provider contribution recipes, review checklist |
| `docs/ROADMAP.md` | Rewritten to reflect actual status |
| `docs/audits/README.md` | New — audit runbook + sandbox negative-test coverage |
| Module headers | Added `//!` headers to `auth/`, `config/`, `orchestrator/`, `providers/`, `tui/` + all `tui/*.rs` (8 files) — every module now documents its role |

---

## 5. METRICS

| Metric | Value |
|--------|-------|
| Rust source | 24,354 lines / 152 tracked files |
| Registered tools | 59 (schema-validated) |
| Unit tests | 282 — all green |
| Integration tests | 29 (6 domain files) — all green |
| Clippy `-D warnings` | 0 findings |
| `cargo build` (debug + release) | 0 errors, 0 warnings |
| `cargo fmt --check` | clean |
| `npm run build` | EXIT 0 (5.97 s) |
| Trailing whitespace / CRLF / TS `any` | 0 / 0 / 0 |
| Shell scripts `bash -n` | all clean |
| Legacy naming (`diod`, `intentos`) | 0 hits |
| TODO/FIXME leftovers | 0 real markers (only EditorPanel's intentional diagnostic regex + `.tmpXXX` doc examples) |
| Secrets in history | none (npm `mdast` package names are false positives) |
| Docs | 1,138 lines across 9 documents |

---

## 6. GATE RESULTS (Phase 9)

| # | Gate | Result |
|---|------|--------|
| 9.1 | `cargo build --release` | ✅ EXIT 0, 0 warnings |
| 9.2 | `cargo test` | ✅ 311 passed (282 unit + 29 integration) |
| 9.3 | `cargo clippy --all-targets -- -D warnings` | ✅ 0 findings |
| 9.4 | `cargo fmt --all --check` | ✅ clean |
| 9.5 | `npm run build` | ✅ EXIT 0 |
| 9.6 | `bash -n` all shell scripts | ✅ clean (shellcheck not installed — needs `sudo pacman -S shellcheck`; documented in `docs/audits/`) |
| 9.7 | `grep -ri "diod\|intentos"` | ✅ 0 hits |
| 9.8 | `grep -rn "TODO\|FIXME\|XXX\|HACK" agent/src frontend/src` | ✅ 0 real leftovers |
| 9.9 | Cross-reference audit (14 boot-chain files) | ✅ all present |
| 9.10 | Boot-chain dry run (sway exec → supervisor → launch) | ✅ all refs resolve |

---

## 7. GOOGLE ENGINEER TEST — 10-step walkthrough

A senior engineer cloning this repo and reading in order can fully understand
the architecture in ~30 minutes:

1. **`README.md`** (3 min) — what DIAD is, the two surfaces, the mermaid
   architecture diagram, provider matrix.
2. **`docs/ARCHITECTURE.md`** (5 min) — boot chain from GRUB to the daemon,
   the versioned WebSocket protocol, data flows.
3. **`agent/Cargo.toml`** (1 min) — 28 deps, all justified; nothing exotic.
4. **`agent/src/lib.rs` + `main.rs`** (2 min) — seven top-level modules with
   `//!` headers explaining each; CLI flags documented.
5. **`agent/src/core/mod.rs`** (5 min) — "one brain, two faces": daemon owns
   logic, server is pure transport, protocol.rs is the contract.
6. **`agent/src/tools/`** (5 min) — domain-grouped capability surface;
   `registry.rs` binds 59 schema-validated tools to handlers; the
   `process.rs` security contract is a single readable function.
7. **`agent/src/providers/`** (3 min) — one trait, native adapters for
   anthropic/gemini/qwen, one shared OpenAI-compatible path for everything
   else.
8. **`agent/src/orchestrator/` + `auth/` + `config/`** (3 min) — intent
   routing, keyring vault, layered config — each a single small module.
9. **`frontend/src/`** (3 min) — `components/` primitives, `panels/` views,
   `services/` clients, `stores/` state, `hooks/`; import graph is shallow.
10. **`agent/tests/`** (3 min) — 6 domain test files + sandbox negative suite
    prove the security model and the app behavior without reading the whole
    source.

Total: ~33 minutes, no dead ends, no dead code, no orphaned files.

---

*Every phase committed atomically with descriptive messages; all 12 commits on
`main` replay cleanly from the `e9809cf` checkpoint.*

**SANITIZATION COMPLETE — AWAITING REVIEW**
