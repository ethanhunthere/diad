# Contributing to DIAD

## Ground rules

- **No dead code.** Every file, function, and dependency must be referenced or
  be removed. The CI gates in `Makefile` (`check`, `lint`, `test`) enforce this.
- **No `any` in TypeScript**, no `unsafe` in Rust, no unvalidated input.
- **Sandbox first.** All filesystem access goes through `ToolRegistry` path
  checks; process spawning through `spawn_process`'s mandatory security
  contract (see `agent/src/tools/system/process.rs`).
- **Google-grade hygiene:** `cargo fmt` on every change, `clippy -D warnings`
  clean, tests green, `npm run build` clean.

## Repository layout

```
diados/
├── agent/          # Rust daemon + TUI client (crate `diad-agent`)
│   ├── src/
│   │   ├── auth/           # keyring vault, oauth
│   │   ├── config/         # layered TOML config
│   │   ├── core/           # daemon, WS protocol, REST, PTY, server
│   │   ├── orchestrator/   # intent router, sessions, agent loop
│   │   ├── providers/      # LLM adapters (anthropic/gemini/qwen/openai…)
│   │   ├── tools/          # capability surface (files/hardware/network/system)
│   │   └── tui/            # ratatui client
│   └── tests/              # domain-split integration tests
├── frontend/       # React + Vite GUI (built to frontend/dist, embedded)
├── os/             # archiso profile (sway, systemd, setup scripts)
├── docs/           # architecture, security, testing, roadmap, audits
├── Makefile        # build/test/iso targets
└── install.sh      # system installer
```

## Adding a tool

1. Implement the handler as a free function in the right domain module under
   `agent/src/tools/` (e.g. `files/filesystem.rs`).
2. Register it in `agent/src/tools/registry.rs` with a JSON schema for its
   parameters (Zod-level validation lives here).
3. If it touches the filesystem, ensure the path goes through
   `sandbox_allowed_paths()` / `is_path_allowed()`.
4. If it spawns a process, use `system::process::spawn_process` (banned-shell
   checks are enforced there).
5. Add a unit test next to the implementation and a negative test to
   `agent/tests/tools_sandbox_tests.rs` if it handles untrusted input.

## Adding a provider

1. Implement `providers::traits::LlmProvider` (streaming).
2. If it is OpenAI-compatible, extend `providers/openai_compat.rs` — most
   providers never need a new file.
3. Register the default model/base URL in the provider table (config + GUI
   model picker) and the README provider matrix.

## Committing

- One logical change per commit, prefixed with the subsystem, e.g.
  `sanitize(phase4): …`, `feat(tools): …`, `fix(daemon): …`.
- Run `make check` (fmt + lint + clippy) and `make test` before committing.
- Never commit credentials, keys, or `work/`/`out/` artifacts (gitignored).

## Review checklist

- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` green (282 + 29)
- [ ] `npm run build` clean (if frontend touched)
- [ ] No new warnings, no dead code, no `any`/`unsafe`
- [ ] Docs updated (README provider matrix, ARCHITECTURE if surfaces changed)
