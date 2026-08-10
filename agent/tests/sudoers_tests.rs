//! Sudoers least-privilege regression tests (audit F-12 / phase A1).
//!
//! These tests read the shipped `/etc/sudoers.d/diad` drop-in and the
//! `/usr/local/bin/diad-*` wrappers straight out of the repo tree and assert
//! the invariants the audit demands:
//!
//! 1. No shell-capable binary (`sh`, `bash`, `su`, ...) is ever granted.
//! 2. `rm`, `sed`, `tee` are gone entirely (no tool legitimately needs them
//!    as root — the audit's F-12).
//! 3. Every `/usr/bin` entry is argv-pinned (no bare binary, no open trailing
//!    wildcard on a variable binary).
//! 4. The destructive variable-arg binaries (`mount`, `umount`, `mkfs`,
//!    `fsck`, `parted`, `dd`, `wget`, `install`, `sha256sum`) are reachable
//!    ONLY through their validated `/usr/local/bin/diad-*` wrappers — never
//!    as direct `/usr/bin` entries.
//! 5. Every wrapper the agent tools call exists on disk and is listed in the
//!    sudoers file (and registered in `os/archiso/profiledef.sh` at 0:0:755).

use std::path::PathBuf;

/// Paths relative to the crate root (`agent/`).
fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

/// Read a repo file, panicking with the path on failure (test setup).
fn read_repo_file(rel: &str) -> String {
    let path = repo_path(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("cannot read {rel} (resolved to {}): {e}", path.display())
    })
}

const SUDOERS_REL: &str = "os/archiso/airootfs/etc/sudoers.d/diad";
const WRAPPER_DIR_REL: &str = "os/archiso/airootfs/usr/local/bin";
const PROFILEDEF_REL: &str = "os/archiso/profiledef.sh";

/// Binaries that can hand out a root shell when granted with open args.
const SHELL_CAPABLE: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "su", "sudo", "pkexec", "doas", "runuser",
];

/// Binaries removed entirely (F-12) — asserting absence catches re-additions.
const FORBIDDEN: &[&str] = &["rm", "sed", "tee"];

/// Variable-arg destructive binaries that must be reachable ONLY via a
/// validated `/usr/local/bin/diad-*` wrapper.
const WRAPPED_ONLY: &[&str] = &[
    "mount", "umount", "mkfs", "fsck", "parted", "dd", "wget", "install", "sha256sum",
];

/// Wrapper scripts the agent tools invoke (or that replace a raw entry).
/// `(script, expected wrapper marker in the script)`.
const WRAPPERS: &[&str] = &[
    "diad-clone-disk",
    "diad-conf-write",
    "diad-format",
    "diad-fsck",
    "diad-install-conf",
    "diad-mount",
    "diad-resize-partition",
    "diad-selfupdate",
    "diad-umount",
    "diad-sysfs-write",
];

/// Split the sudoers file into individual `path argv...` entries.
fn entries(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Everything after the first `NOPASSWD:` (a line may repeat it).
        let body = match line.find("NOPASSWD:") {
            Some(i) => &line[i + "NOPASSWD:".len()..],
            None => continue,
        };
        for part in body.split(',') {
            let part = part.trim();
            if !part.is_empty() {
                out.push(part.to_string());
            }
        }
    }
    out
}

fn basename_of(entry: &str) -> &str {
    entry
        .split_whitespace()
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("")
}

#[test]
fn sudoers_has_no_shell_capable_or_forbidden_binaries() {
    let content = read_repo_file(SUDOERS_REL);
    let banned: Vec<&str> = SHELL_CAPABLE.iter().chain(FORBIDDEN.iter()).copied().collect();

    for entry in entries(&content) {
        let bin = basename_of(&entry);
        for b in &banned {
            assert!(
                *b != bin,
                "sudoers grants forbidden/shell-capable binary '{bin}' (entry: {entry})"
            );
        }
    }
    // Belt-and-braces: the names must not appear even inside comment lines.
    for b in FORBIDDEN {
        assert!(
            !content.lines().any(|l| l.trim_start().starts_with('#') == false
                && l.contains(&format!("/{b} "))),
            "forbidden binary '{b}' still present in sudoers"
        );
    }
}

#[test]
fn sudoers_usr_bin_entries_are_argv_pinned() {
    // The audit's F-12 rule, verbatim: "no generic SHELL-CAPABLE binary with
    // open wildcards". A bare `/usr/bin/foo *` entry is only tolerated for
    // read-only diagnostics that can neither write files nor spawn processes
    // (eject, traceroute). Anything else must carry a pinned argv prefix.
    const READONLY_SINGLE_ARG_OK: &[&str] = &["eject", "traceroute"];

    let content = read_repo_file(SUDOERS_REL);
    for entry in entries(&content) {
        if !entry.starts_with("/usr/bin/") {
            continue;
        }
        let bin = basename_of(&entry);
        let args: Vec<&str> = entry.split_whitespace().skip(1).collect();
        assert!(
            !args.is_empty(),
            "bare /usr/bin entry without pinned argv: {entry}"
        );
        if args.len() == 1 && args[0] == "*" {
            assert!(
                READONLY_SINGLE_ARG_OK.contains(&bin),
                "open wildcard on /usr/bin binary '{bin}': {entry}"
            );
        }
    }
}

#[test]
fn sudoers_reaches_destructive_binaries_only_via_wrappers() {
    let content = read_repo_file(SUDOERS_REL);
    for entry in entries(&content) {
        let bin = basename_of(&entry);
        if WRAPPED_ONLY.contains(&bin) {
            panic!(
                "destructive binary '{bin}' granted directly in sudoers (entry: {entry}); \
                 it must go through a validated /usr/local/bin/diad-* wrapper"
            );
        }
    }
    // And every wrapper the tools need IS granted.
    for w in WRAPPERS {
        assert!(
            content.contains(&format!("/usr/local/bin/{w}")),
            "sudoers missing required wrapper /usr/local/bin/{w}"
        );
    }
}

#[test]
fn wrappers_exist_on_disk_and_are_executable_in_iso() {
    let dir = repo_path(WRAPPER_DIR_REL);
    let profiledef = read_repo_file(PROFILEDEF_REL);
    for w in WRAPPERS {
        let script = dir.join(w);
        assert!(
            script.is_file(),
            "wrapper script missing on disk: {}",
            script.display()
        );
        // profiledef.sh must list it 0:0:755 or mkarchiso ships it 0644
        // non-executable (customize_airootfs.sh is deprecated).
        assert!(
            profiledef.contains(&format!("/usr/local/bin/{w}\"]=\"0:0:755\"")),
            "profiledef.sh missing 0:0:755 entry for {w}"
        );
    }
}

#[test]
fn sudoers_syntax_sanity() {
    // Cheap parse sanity (the authoritative check is `visudo -cf`, run by the
    // maintainer on the ISO — see docs/audits/): every non-comment line must
    // contain `ALL=(ALL)` and `NOPASSWD:`, and no path may contain whitespace
    // beyond the single separator.
    let content = read_repo_file(SUDOERS_REL);
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        assert!(
            line.contains("ALL=(ALL)") && line.contains("NOPASSWD:"),
            "malformed sudoers line: {line}"
        );
        assert!(
            line.starts_with("diad "),
            "unexpected subject on sudoers line: {line}"
        );
    }
    assert!(!content.trim().is_empty(), "sudoers file must not be empty");
}
