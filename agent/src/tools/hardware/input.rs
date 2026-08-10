//! Laptop input / media-key control tools — keyboard backlight, volume,
//! airplane mode, and media playback.
//!
//! These are the programmatic counterparts of the Sway keybindings that
//! `diad-hardware-setup.sh` enables on laptops (see the `MEDIA_KEYS_BEGIN` /
//! `MEDIA_KEYS_END` block in `/etc/sway/config`).
//!
//! Privileged commands (brightnessctl writes, nmcli radio) run through
//! `sudo` — whitelisted in `/etc/sudoers.d/diad`.

use super::ToolResult;
use crate::tools::system::exec;

// ===========================================================================
// Keyboard backlight (brightnessctl)
// ===========================================================================

/// Increase the keyboard backlight by 10%.
pub fn keyboard_brightness_up() -> anyhow::Result<ToolResult> {
    exec::run_privileged(&["brightnessctl", "set", "+10%"])
}

/// Decrease the keyboard backlight by 10%.
pub fn keyboard_brightness_down() -> anyhow::Result<ToolResult> {
    exec::run_privileged(&["brightnessctl", "set", "-10%"])
}

// ===========================================================================
// Volume (WirePlumber / wpctl)
// ===========================================================================

/// Raise the default sink volume by 5%.
pub fn volume_up() -> anyhow::Result<ToolResult> {
    exec::run(&["wpctl", "set-volume", "@DEFAULT_SINK@", "5%+"])
}

/// Lower the default sink volume by 5%.
pub fn volume_down() -> anyhow::Result<ToolResult> {
    exec::run(&["wpctl", "set-volume", "@DEFAULT_SINK@", "5%-"])
}

/// Toggle mute on the default sink.
pub fn volume_mute() -> anyhow::Result<ToolResult> {
    exec::run(&["wpctl", "set-mute", "@DEFAULT_SINK@", "toggle"])
}

// ===========================================================================
// Airplane mode (NetworkManager / nmcli)
// ===========================================================================

/// Toggle airplane mode: `enabled = true` turns the Wi-Fi radio **off**,
/// `enabled = false` turns it back on.
pub fn airplane_mode(enabled: bool) -> anyhow::Result<ToolResult> {
    if enabled {
        exec::run_privileged(&["nmcli", "radio", "wifi", "off"])
    } else {
        exec::run_privileged(&["nmcli", "radio", "wifi", "on"])
    }
}

/// Build the exact nmcli argument vector for an airplane-mode request.
/// Exposed for unit testing (avoids spawning real commands in tests).
#[cfg(test)]
fn airplane_mode_args(enabled: bool) -> Vec<&'static str> {
    if enabled {
        vec!["nmcli", "radio", "wifi", "off"]
    } else {
        vec!["nmcli", "radio", "wifi", "on"]
    }
}

// ===========================================================================
// Media playback (playerctl)
// ===========================================================================

/// Toggle play/pause on the active media player.
pub fn media_play_pause() -> anyhow::Result<ToolResult> {
    exec::run(&["playerctl", "play-pause"])
}

/// Skip to the next track on the active media player.
pub fn media_next() -> anyhow::Result<ToolResult> {
    exec::run(&["playerctl", "next"])
}

/// Go back to the previous track on the active media player.
pub fn media_previous() -> anyhow::Result<ToolResult> {
    exec::run(&["playerctl", "previous"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_airplane_mode_on_turns_wifi_off() {
        assert_eq!(
            airplane_mode_args(true),
            vec!["nmcli", "radio", "wifi", "off"]
        );
    }

    #[test]
    fn test_airplane_mode_off_turns_wifi_on() {
        assert_eq!(
            airplane_mode_args(false),
            vec!["nmcli", "radio", "wifi", "on"]
        );
    }
}
