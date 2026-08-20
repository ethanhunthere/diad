# A — Cargo Tests (`/home/f0id/diados/agent`)

- Date: 2026-08-18 · Toolchain: cargo/rustc 1.97.1 (Arch Linux)
- Method: `cargo test` run **3× strictly sequential**, full logs in `/tmp/cargo-test-run{1,2,3}.log`
- No repo files modified (report file is the only write).

## Per-run results (identical across all 3 runs)

| Test binary | Passed | Failed |
|---|---|---|
| lib unittests (`src/lib.rs`, diad_agent) | 347 | 0 |
| `main.rs` unittests | 0 | 0 |
| `config_tests` | 2 | 0 |
| `orchestrator_tests` | 4 | 0 |
| `providers_tests` | 1 | 0 |
| `sudoers_tests` | 5 | 0 |
| `supervisor_tests` | 5 | 0 |
| `tools_sandbox_tests` | 8 | 0 |
| `tools_system_tests` | 4 | 0 |
| `tui_app_tests` | 10 | 0 |
| doc-tests | 0 | 0 |
| **Total per run** | **386** | **0** |

All 3 runs exited 0. Per-binary counts were byte-identical run-to-run
(lib wall time: 6.38s / 5.92s / 5.70s).

## Flake determination

- No test failed in any of the 3 full parallel runs, so no failure-isolation
  reruns were required.
- The previously suspected `auth::vault::tests::test_vault_roundtrip`:
  - appeared and passed (`ok`) in **all 3** full parallel runs;
  - rerun isolated `cargo test --lib auth::vault::tests::test_vault_roundtrip -- --nocapture`:
    **1 passed, 0 failed** (346 filtered out), 0.06s.
- Conclusion: the suspected gnome-keyring concurrency flake was **not
  reproduced** on this host in this session. Verdict for this test:
  stable across 3 parallel + 1 isolated runs (single-session evidence;
  a prior full-suite failure was reported historically but did not recur).

## Release build

- `cargo build --release`: **clean** — `Finished release profile [optimized]
  target(s) in 4m 19s`, exit 0, **0 warnings** in output.

## Overall verdict

**PASS** — 3/3 full-suite runs green (386 tests each, 0 failures),
suspected vault flake not reproduced, release build compiles cleanly.
