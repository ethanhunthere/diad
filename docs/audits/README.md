# Security & Sanitization Audit Trail

This directory records the security and code-health audits performed on the
DIAD codebase. Each entry links the audit to the commit range it covered.

## How to run the audits

```bash
# 1. Dependency audit (requires network; run in agent/)
cd agent && cargo audit

# 2. Clippy as a gate (must exit 0)
cargo clippy --all-targets -- -D warnings

# 3. Full test suite (must exit 0)
cargo test

# 4. Frontend type-check + build (must exit 0)
cd ../frontend && npm run build

# 5. Shell syntax check on all os/ scripts
for s in os/archiso/airootfs/usr/local/bin/*.sh; do bash -n "$s"; done
```

## Audit log

| Date       | Audit                                            | Result        |
|------------|--------------------------------------------------|---------------|
| 2026-08-09 | Pre-sanitization checkpoint commit `e9809cf`     | Baseline set  |
| 2026-08-09 | Phase 1–3: dead files, dead code, dependencies   | All gates 0   |
| 2026-08-09 | Phase 4: module restructure (core/tools/frontend)| Build/clippy 0|
| 2026-08-09 | Phase 5: test restructure + sandbox negative tests | 282 + 29 green |

## Sandbox negative-test coverage

`agent/tests/tools_sandbox_tests.rs` asserts the sandbox *rejects*:

- `read_file` of `/etc/passwd` (outside allowed paths)
- `write_file` / `chmod_file` under `/etc/`
- path-traversal (`../../etc/passwd`)
- `spawn_process` with shells or privilege-elevation binaries
  (`sh`, `sudo`, …) and with an empty command
- `http_get` with a non-URL (fails before any network I/O)

## Policy

- Any future change that loosens a sandbox rule must add a negative test in
  the same commit.
- Secrets are never committed; the keyring is the only store for API keys.
- `cargo audit` is expected to be run before every tagged release.
