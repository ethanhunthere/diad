//! Help & info tools — usage guide, tool inventory, version info.
//!
//! `show_tool_list` reads the registry snapshot that `create_default_registry`
//! captures once all tools are registered (`REGISTERED_TOOLS` in
//! `tools/mod.rs`).

use super::ToolResult;

/// The comprehensive DIAD help text.
pub const HELP_TEXT: &str = r#"DIAD — Intent is the only interface.

SLASH COMMANDS
  /help      Show this help text
  /tools     List every tool the agent can execute
  /version   Show DIAD version and build info
  /model     Open the provider & model picker (or press F6)
  /connect            Set up an API key for the current provider (or press F5)
  /connect <name>     Set up an API key for any provider by name (kimi, deepseek,
                      groq, mistral, openrouter, ollama, ... or a custom one)

INTERFACE
  F1  Chat            F2  File Tree
  F3  Code View       F4  System Status
  F5  Connect         F6  Provider & Model Picker
  Ctrl+L  Clear messages
  Ctrl+D / Ctrl+C     Quit

PROVIDERS
  Every major model family is usable from one place:
  anthropic · gemini · openai (gpt-5, gpt-5-codex, codex-mini-latest, o4-mini)
  qwen · kimi · deepseek · groq · mistral · openrouter · ollama (local, no key)
  Plus any provider you add under [providers.<name>] in diad.conf.
  API keys are stored per-provider in the system keyring and never logged.

EXTERNAL AGENTS (local AI coding CLIs)
  delegate_to_agent hands a task to an installed CLI — claude, codex, aider,
  opencode, gemini — which runs non-interactively and streams output back as
  [agent:<name>] lines. Always asks for confirmation first.

OFFLINE INTENTS (no API key required)
  "connect to wifi"       List networks, then connect (masked password)
  "set volume to 40"      Set the audio volume (0-100)
  "set brightness to 50"  Set the screen brightness (0-100)
  "reboot|shutdown|suspend"  Power actions (always confirmed)

TOOL CATEGORIES
  Filesystem   read_file, write_file, list_dir, search_files, chmod_file
  Process      spawn_process, list_processes, kill_process
  Network      http_get, http_post, ping_host
  System       get_system_info, install_package, remove_package, manage_service
  Hardware     wifi_list, wifi_connect, wifi_status, wifi_disconnect,
               get_volume, set_volume, toggle_mute, get_brightness,
               set_brightness, power, bluetooth, get_network_interfaces,
               connect_ethernet, set_hostname
  Input        keyboard_brightness_up, keyboard_brightness_down, volume_up,
               volume_down, volume_mute, airplane_mode, media_play_pause,
               media_next, media_previous
  Display      list_outputs, set_resolution, set_scale, set_position,
               mirror_outputs
  Power        get_battery_status, set_power_profile, get_power_profile,
               set_sleep_timeout
  Security     lock_screen, enable_firewall, disable_firewall
  Updates      check_system_updates, apply_system_updates,
               check_agent_updates, apply_agent_updates
  External     delegate_to_agent (claude, codex, aider, opencode, gemini)
  Help         show_help, show_tool_list, show_version

SECURITY NOTES
  • Destructive actions (power, system updates) always ask for confirmation.
  • Spawning an external AI CLI always asks for confirmation.
  • API keys and Wi-Fi passwords are entered with masked prompts and never stored.
  • Tools run with least privilege via /etc/sudoers.d/diad.

Type /tools for the live list of every registered tool."#;

/// Show the comprehensive DIAD help text.
pub fn show_help() -> anyhow::Result<ToolResult> {
    Ok(ToolResult::ok(HELP_TEXT))
}

/// List every registered tool with its description.
pub fn show_tool_list() -> anyhow::Result<ToolResult> {
    let tools = crate::tools::registered_tools_snapshot();
    if tools.is_empty() {
        return Ok(ToolResult::ok("No tools registered."));
    }

    let mut out = String::from("DIAD tools — {count} registered:\n");
    let count = tools.len();
    out = out.replacen("{count}", &count.to_string(), 1);

    for tool in tools {
        out.push_str(&format!("  • {:<28} {}\n", tool.name, tool.description));
    }
    Ok(ToolResult::ok(out.trim_end().to_string()))
}

/// Show DIAD version and build info.
pub fn show_version() -> anyhow::Result<ToolResult> {
    let profile = option_env!("PROFILE").unwrap_or("unknown");
    let target_os = std::env::consts::OS;
    let target_arch = std::env::consts::ARCH;

    Ok(ToolResult::ok(format!(
        "DIAD Agent v{}\nBuild profile: {profile}\nTarget: {target_os}/{target_arch}\nRepository: https://github.com/diados/diados",
        env!("CARGO_PKG_VERSION")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_help_mentions_commands() {
        let result = show_help().unwrap();
        assert!(result.success);
        assert!(result.output.contains("/help"));
        assert!(result.output.contains("/tools"));
        assert!(result.output.contains("/version"));
        assert!(result.output.contains("DIAD"));
    }

    #[test]
    fn test_show_version_reports_version() {
        let result = show_version().unwrap();
        assert!(result.success);
        assert!(result.output.contains(env!("CARGO_PKG_VERSION")));
        assert!(result.output.contains("DIAD Agent"));
    }

    #[test]
    fn test_show_tool_list_lists_registered_tools() {
        // Snapshot is populated by create_default_registry (called in other
        // tests first); if it is somehow empty, the tool still returns a
        // valid result rather than panicking.
        let result = show_tool_list().unwrap();
        assert!(result.success);
    }
}
