//! Validated "compositor preference" intents (audit F-15 / phase A2.4).
//!
//! THE LAW: every configurable thing must be reachable from the GUI, the TUI,
//! or chat intent. These three functions are that surface for compositor
//! internals — sway keybindings, the perf-tier override, and the waybar
//! layout variant. They all write through the `/usr/local/bin/diad-conf-write`
//! wrapper (sudoers least privilege), never via raw file writes, and every
//! value is validated here AND in the wrapper (defense in depth).

use super::ToolResult;
use crate::tools::system::exec;

/// Valid sway keysym characters — must stay in sync with the wrapper's
/// `KEYSYM_RE` in `os/archiso/airootfs/usr/local/bin/diad-conf-write`.
fn valid_keysym(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '_' | ':' | '-'))
}

/// Remap a sway keybinding (`from` keysym → `to` workspace/keysym).
///
/// Persists to `/etc/diad/sway-overrides.conf` (included by the sway config)
/// and applies immediately via `swaymsg`. Injection is rejected both here
/// and in the wrapper.
pub fn set_keybinding(from: &str, to: &str) -> anyhow::Result<ToolResult> {
    if !valid_keysym(from) || !valid_keysym(to) {
        return Ok(ToolResult::err(
            "set_keybinding: invalid keysym (allowed: A-Z a-z 0-9 + _ : -)",
        ));
    }
    exec::run_privileged(&[
        "/usr/local/bin/diad-conf-write",
        "keybinding",
        from,
        to,
    ])
}

/// Override the performance tier (`/etc/diad/perf-tier`).
///
/// Valid tiers: ULTRA_LOW | LOW | MID | HIGH | ULTRA.
pub fn set_perf_tier(tier: &str) -> anyhow::Result<ToolResult> {
    match tier {
        "ULTRA_LOW" | "LOW" | "MID" | "HIGH" | "ULTRA" => {}
        _ => {
            return Ok(ToolResult::err(
                "set_perf_tier: invalid tier (ULTRA_LOW|LOW|MID|HIGH|ULTRA)",
            ))
        }
    }
    exec::run_privileged(&[
        "/usr/local/bin/diad-conf-write",
        "perf-tier",
        tier,
    ])
}

/// Switch the waybar layout variant (`compact` or `full`).
///
/// Installs `/etc/waybar/config.{mode}` over `/etc/waybar/config` and pokes
/// waybar to reload (the supervisor watchdog guarantees exactly one restart).
pub fn set_waybar_mode(mode: &str) -> anyhow::Result<ToolResult> {
    match mode {
        "compact" | "full" => {}
        _ => return Ok(ToolResult::err("set_waybar_mode: invalid mode (compact|full)")),
    }
    exec::run_privileged(&[
        "/usr/local/bin/diad-conf-write",
        "waybar-mode",
        mode,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybinding_rejects_injection() {
        // Shell metacharacters, whitespace, and path tricks must never
        // survive validation — the wrapper also rejects them, but the tool
        // must fail fast before ever touching sudo.
        for evil in [
            "x; rm -rf /",
            "$(touch /tmp/pwned)",
            "`id`",
            "Mod4+x | sh",
            "a\nb",
            "&& reboot",
            "Super+; rm",
            "../../etc/passwd",
            "x y",
            "y\"x",
        ] {
            let from = valid_keysym(evil);
            let to = valid_keysym(evil);
            assert!(!from && !to, "keysym accepted injection: {evil:?}");
        }
    }

    #[test]
    fn keybinding_accepts_legit_keysyms() {
        for ok in ["Mod4+Return", "Ctrl+Shift+c", "XF86AudioRaiseVolume", "Super_L", "1", "P"] {
            assert!(valid_keysym(ok), "rejected valid keysym: {ok}");
        }
    }

    #[test]
    fn perf_tier_validate_allowlist() {
        for bad in ["GOD", "medium", "MID ", "", "HIGH\nreboot", "ultra"] {
            let result = set_perf_tier(bad).unwrap();
            assert!(!result.success, "accepted invalid tier: {bad:?}");
        }
    }

    #[test]
    fn waybar_mode_validate_allowlist() {
        for bad in ["huge", "compact;reboot", "", "FULL"] {
            let result = set_waybar_mode(bad).unwrap();
            assert!(!result.success, "accepted invalid mode: {bad:?}");
        }
    }
}
