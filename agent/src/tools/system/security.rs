//! Security tools — screen lock and firewall control.
//!
//! The firewall is a systemd oneshot unit (`diad-firewall.service`) that
//! applies `/etc/nftables.conf` (default-drop inbound policy).

use super::ToolResult;
use crate::tools::system::exec;

/// Lock the screen (solid black) via swaylock.
pub fn lock_screen() -> anyhow::Result<ToolResult> {
    exec::run(&["swaylock", "-f", "-c", "000000"])
}

/// Enable the DIAD nftables firewall (oneshot systemd unit, starts now).
pub fn enable_firewall() -> anyhow::Result<ToolResult> {
    exec::run_privileged(&["systemctl", "enable", "--now", "diad-firewall"])
}

/// Disable the DIAD nftables firewall.
pub fn disable_firewall() -> anyhow::Result<ToolResult> {
    exec::run_privileged(&["systemctl", "disable", "--now", "diad-firewall"])
}
