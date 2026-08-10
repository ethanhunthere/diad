//! Supervisor regression tests (audit F-13 / F-14, phase A3).
//!
//! These tests read the shipped `diad-supervisor.sh` straight out of the
//! repo tree and assert the invariants the audit demands:
//!
//! 1. F-13 — size-capped log rotation: a `rotate_logs` helper that only
//!    touches files over 1 MiB, keeps exactly one `.1` backup, and uses
//!    copytruncate (not mv) so the daemon's open fd keeps writing to the
//!    live file. Covers both `~/.diad/logs/*.log` and `~/.diad/daemon.log`.
//! 2. F-14 — the supervisor is a long-running loop (no more `exit 0` after
//!    setup) with a waybar watchdog at a 10 s cadence, relaunching waybar
//!    with the exact same flags Sway uses (`-c /etc/waybar/config`).
//! 3. Syntax gate — `bash -n` must pass (a broken supervisor kills the
//!    whole session at boot).

use std::path::PathBuf;
use std::process::Command;

/// Paths relative to the crate root (`agent/`).
fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

const SUPERVISOR_REL: &str = "os/archiso/airootfs/usr/local/bin/diad-supervisor.sh";

fn supervisor_script() -> String {
    let path = repo_path(SUPERVISOR_REL);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("cannot read {SUPERVISOR_REL} (resolved to {}): {e}", path.display())
    })
}

/// A complete `rotate_logs` function body, extracted for logic assertions.
fn rotate_logs_body(script: &str) -> String {
    let start = script
        .find("rotate_logs()")
        .unwrap_or_else(|| panic!("rotate_logs() not defined"));
    let open = script[start..].find('{').unwrap() + start;
    // Balanced-brace scan so we get exactly the function body.
    let mut depth = 0usize;
    let mut end = open;
    for (i, c) in script[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    script[start..end].to_string()
}

#[test]
fn supervisor_rotates_logs_with_size_cap_and_single_backup() {
    let body = rotate_logs_body(&supervisor_script());

    // 1 MiB cap, expressed explicitly.
    assert!(
        body.contains("1048576"),
        "rotation must cap at 1 MiB (1048576 bytes): {body}"
    );

    // Keep exactly one backup with the `.1` suffix.
    assert!(
        body.contains("\"${log}.1\""),
        "rotation must keep one backup named <log>.1: {body}"
    );

    // Copytruncate, not mv: the daemon holds an open fd on daemon.log and
    // must keep writing to the LIVE file, not the renamed backup.
    assert!(
        body.contains("cp -f") && body.contains(": > \"${log}\""),
        "rotation must copy then truncate the live file (copytruncate): {body}"
    );

    // Rotation must not touch files under the cap — the stat guard is the
    // whole point (avoid pointless copy traffic every cycle).
    assert!(
        body.contains("-gt ${max_bytes}") || body.contains("> ${max_bytes}"),
        "rotation must skip files under the size cap: {body}"
    );
}

#[test]
fn supervisor_rotation_covers_logs_dir_and_daemon_log() {
    let body = rotate_logs_body(&supervisor_script());
    assert!(
        body.contains("\"${LOG_DIR}\"/*.log"),
        "rotation must scan ~/.diad/logs/*.log: {body}"
    );
    assert!(
        body.contains("${DIAD_HOME}/daemon.log"),
        "rotation must also cover ~/.diad/daemon.log (it lives outside logs/): {body}"
    );
}

#[test]
fn supervisor_runs_a_waybar_watchdog_loop() {
    let script = supervisor_script();

    // F-14: the supervisor must NOT exit after setup — it becomes a loop.
    assert!(
        !script.contains("exit 0"),
        "supervisor must no longer exit 0 after setup (it is the watchdog loop)"
    );
    assert!(
        script.contains("while true"),
        "supervisor must run an infinite watchdog loop"
    );

    // Relaunch check at 10 s cadence, exact-name match so we never match
    // e.g. a shell running `waybar --help`.
    assert!(
        script.contains("pgrep -x waybar"),
        "watchdog must check waybar by exact process name"
    );
    assert!(
        script.contains("sleep 10"),
        "watchdog cadence must be 10 s"
    );

    // Relaunch must use the same flags Sway uses (spec 6.6 / sway config),
    // so the relaunched bar looks identical and honors conf-write changes.
    assert!(
        script.contains("waybar -c /etc/waybar/config -s /etc/waybar/style.css"),
        "watchdog must relaunch waybar with Sway's exact flags"
    );
}

#[test]
fn supervisor_rotates_logs_on_a_cadence_inside_the_loop() {
    let script = supervisor_script();
    // Rotation must be reachable from the steady-state loop, not just at
    // startup — logs grow while the session runs.
    let loop_start = script
        .find("while true")
        .unwrap_or_else(|| panic!("no watchdog loop"));
    let tail = &script[loop_start..];
    assert!(
        tail.contains("rotate_logs"),
        "rotation must be invoked from the watchdog loop"
    );
}

#[test]
fn supervisor_passes_bash_syntax_check() {
    // bash -n is the cheap syntax gate; the ISO's real check is the script
    // running at boot. This test runs the same check on the shipped file.
    let path = repo_path(SUPERVISOR_REL);
    let out = Command::new("bash")
        .arg("-n")
        .arg(&path)
        .output()
        .unwrap_or_else(|e| panic!("cannot run bash -n: {e}"));
    assert!(
        out.status.success(),
        "bash -n failed for {}:\n{}",
        path.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}
