//! Intent Router — classifies user input and routes it to the appropriate handler.
//!
//! The router determines whether a user's input is:
//! - A **chat** message (conversation with the LLM)
//! - A **tool call** (direct system command)
//! - An **edit command** (file creation/modification)
//! - A **system query** (status check, diagnostics)
//! - An **offline intent** (handled locally without any LLM provider)

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Classification of a user's intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Intent {
    /// General conversation — send to LLM
    Chat,
    /// Direct tool invocation (e.g., "list files in ~/Documents")
    ToolCall,
    /// Code or file editing request
    EditCommand,
    /// System status or diagnostic query
    SystemQuery,
}

/// An intent that can be handled locally, entirely without a provider.
///
/// These power the offline fallback: common system-control requests are
/// pattern-matched and executed directly through the hardware tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OfflineIntent {
    /// Wi-Fi: list networks, then optionally connect to one
    Wifi,
    /// Set the volume to a percentage (0-100)
    SetVolume(u32),
    /// Set the brightness to a percentage (0-100)
    SetBrightness(u32),
    /// Power action: "shutdown", "reboot", or "suspend"
    Power(String),
    /// `/help` — show the DIAD help text
    Help,
    /// `/tools` — list every registered tool
    Tools,
    /// `/version` — show DIAD version and build info
    Version,
}

/// Routes user input to the appropriate handler based on intent classification.
pub struct IntentRouter;

impl IntentRouter {
    /// Classify a raw input string into an Intent.
    ///
    /// Uses keyword heuristics to determine the user's intent.
    /// In a full implementation, this would use an LLM classifier or
    /// embedded model for more accurate routing.
    pub fn classify_input(input: &str) -> Intent {
        let lower = input.to_lowercase();

        // Check for edit commands
        let edit_keywords = [
            "edit", "create file", "write file", "modify", "update file",
            "new file", "open file", "change file", "rewrite",
        ];
        if edit_keywords.iter().any(|kw| lower.contains(kw)) {
            return Intent::EditCommand;
        }

        // Check for system queries
        let system_keywords = [
            "status", "diagnostics", "system info", "cpu usage", "memory",
            "disk space", "uptime", "processes", "services", "logs",
        ];
        if system_keywords.iter().any(|kw| lower.contains(kw)) {
            return Intent::SystemQuery;
        }

        // Check for direct tool calls
        let tool_keywords = [
            "list", "read", "write", "create", "delete", "move", "copy",
            "install", "remove", "download", "run", "execute", "search",
            "find", "show", "get",
        ];
        if tool_keywords.iter().any(|kw| lower.starts_with(kw)) {
            return Intent::ToolCall;
        }

        // Default: chat
        Intent::Chat
    }

    /// Classify input into an offline-handleable intent (no provider needed).
    ///
    /// Pattern matching (case-insensitive):
    /// - `/help`, `/tools`, `/version`   → local info commands (exact match)
    /// - `wifi | connect.*network | internet`          → `Wifi`
    /// - `volume <number>`                             → `SetVolume(n)`
    /// - `brightness <number>`                         → `SetBrightness(n)`
    /// - `reboot | shutdown | suspend`                 → `Power(action)`
    ///
    /// Returns `None` when the input does not match a local intent.
    pub fn classify_offline(input: &str) -> Option<OfflineIntent> {
        let lower = input.to_lowercase();

        // Compile patterns once (lazy static)
        static HELP_RE: OnceLock<Regex> = OnceLock::new();
        static TOOLS_RE: OnceLock<Regex> = OnceLock::new();
        static VERSION_RE: OnceLock<Regex> = OnceLock::new();
        static WIFI_RE: OnceLock<Regex> = OnceLock::new();
        static VOLUME_RE: OnceLock<Regex> = OnceLock::new();
        static BRIGHTNESS_RE: OnceLock<Regex> = OnceLock::new();
        static POWER_RE: OnceLock<Regex> = OnceLock::new();

        let help_re = HELP_RE.get_or_init(|| Regex::new(r"^/help$").expect("valid help regex"));
        let tools_re =
            TOOLS_RE.get_or_init(|| Regex::new(r"^/tools$").expect("valid tools regex"));
        let version_re =
            VERSION_RE.get_or_init(|| Regex::new(r"^/version$").expect("valid version regex"));
        let wifi_re = WIFI_RE.get_or_init(|| {
            Regex::new(r"wifi|connect.*network|internet").expect("valid wifi regex")
        });
        let volume_re = VOLUME_RE.get_or_init(|| {
            Regex::new(r"volume[^\d]*(\d+)").expect("valid volume regex")
        });
        let brightness_re = BRIGHTNESS_RE.get_or_init(|| {
            Regex::new(r"brightness[^\d]*(\d+)").expect("valid brightness regex")
        });
        let power_re = POWER_RE.get_or_init(|| {
            Regex::new(r"\b(reboot|shutdown|suspend)\b").expect("valid power regex")
        });

        // Slash commands are exact matches, checked first.
        if help_re.is_match(&lower) {
            return Some(OfflineIntent::Help);
        }
        if tools_re.is_match(&lower) {
            return Some(OfflineIntent::Tools);
        }
        if version_re.is_match(&lower) {
            return Some(OfflineIntent::Version);
        }

        if let Some(caps) = volume_re.captures(&lower) {
            if let Some(value) = caps.get(1) {
                if let Ok(percent) = value.as_str().parse::<u32>() {
                    return Some(OfflineIntent::SetVolume(percent.min(100)));
                }
            }
        }

        if let Some(caps) = brightness_re.captures(&lower) {
            if let Some(value) = caps.get(1) {
                if let Ok(percent) = value.as_str().parse::<u32>() {
                    return Some(OfflineIntent::SetBrightness(percent.min(100)));
                }
            }
        }

        if let Some(caps) = power_re.captures(&lower) {
            if let Some(value) = caps.get(1) {
                let action = value.as_str().to_string();
                return Some(OfflineIntent::Power(action));
            }
        }

        if wifi_re.is_match(&lower) {
            return Some(OfflineIntent::Wifi);
        }

        None
    }

    /// Determine which provider should handle this intent.
    ///
    /// Returns the provider name most suited for the task.
    /// Some providers are better at code, others at general reasoning.
    pub fn route_to_provider(intent: &Intent, default_provider: &str) -> String {
        match intent {
            Intent::EditCommand => {
                // Claude and GPT-4 are strong at code generation
                // Prefer the default unless it's a weaker code model
                default_provider.to_string()
            }
            Intent::SystemQuery => {
                // Any provider can handle system queries
                default_provider.to_string()
            }
            Intent::ToolCall => {
                // Tool use works best with Anthropic or OpenAI
                default_provider.to_string()
            }
            Intent::Chat => {
                // General conversation — use the user's preference
                default_provider.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_chat() {
        assert_eq!(
            IntentRouter::classify_input("Hello, how are you?"),
            Intent::Chat
        );
        assert_eq!(
            IntentRouter::classify_input("What is the meaning of life?"),
            Intent::Chat
        );
    }

    #[test]
    fn test_classify_tool_call() {
        assert_eq!(
            IntentRouter::classify_input("list files in /tmp"),
            Intent::ToolCall
        );
        assert_eq!(
            IntentRouter::classify_input("read the README"),
            Intent::ToolCall
        );
    }

    #[test]
    fn test_classify_edit() {
        assert_eq!(
            IntentRouter::classify_input("create file hello.rs with a main function"),
            Intent::EditCommand
        );
        assert_eq!(
            IntentRouter::classify_input("edit the config file"),
            Intent::EditCommand
        );
    }

    #[test]
    fn test_classify_system() {
        assert_eq!(
            IntentRouter::classify_input("system info"),
            Intent::SystemQuery
        );
        assert_eq!(
            IntentRouter::classify_input("cpu usage"),
            Intent::SystemQuery
        );
    }

    #[test]
    fn test_route_to_provider() {
        let provider = IntentRouter::route_to_provider(&Intent::Chat, "anthropic");
        assert_eq!(provider, "anthropic");
    }

    #[test]
    fn test_classify_offline_wifi() {
        assert_eq!(
            IntentRouter::classify_offline("can you connect to the wifi"),
            Some(OfflineIntent::Wifi)
        );
        assert_eq!(
            IntentRouter::classify_offline("I need internet"),
            Some(OfflineIntent::Wifi)
        );
        assert_eq!(
            IntentRouter::classify_offline("connect to the network please"),
            Some(OfflineIntent::Wifi)
        );
        assert_eq!(
            IntentRouter::classify_offline("show available wifi networks"),
            Some(OfflineIntent::Wifi)
        );
    }

    #[test]
    fn test_classify_offline_volume() {
        assert_eq!(
            IntentRouter::classify_offline("set volume to 40"),
            Some(OfflineIntent::SetVolume(40))
        );
        assert_eq!(
            IntentRouter::classify_offline("volume 100"),
            Some(OfflineIntent::SetVolume(100))
        );
        assert_eq!(
            IntentRouter::classify_offline("turn the volume up"),
            None
        );
    }

    #[test]
    fn test_classify_offline_brightness() {
        assert_eq!(
            IntentRouter::classify_offline("set brightness to 50"),
            Some(OfflineIntent::SetBrightness(50))
        );
        assert_eq!(
            IntentRouter::classify_offline("brightness 25"),
            Some(OfflineIntent::SetBrightness(25))
        );
    }

    #[test]
    fn test_classify_offline_power() {
        assert_eq!(
            IntentRouter::classify_offline("reboot the machine"),
            Some(OfflineIntent::Power("reboot".into()))
        );
        assert_eq!(
            IntentRouter::classify_offline("shutdown"),
            Some(OfflineIntent::Power("shutdown".into()))
        );
        assert_eq!(
            IntentRouter::classify_offline("please suspend the system"),
            Some(OfflineIntent::Power("suspend".into()))
        );
    }

    #[test]
    fn test_classify_offline_slash_commands() {
        assert_eq!(
            IntentRouter::classify_offline("/help"),
            Some(OfflineIntent::Help)
        );
        assert_eq!(
            IntentRouter::classify_offline("/tools"),
            Some(OfflineIntent::Tools)
        );
        assert_eq!(
            IntentRouter::classify_offline("/version"),
            Some(OfflineIntent::Version)
        );
        // Not a slash command — falls through to other patterns.
        assert_eq!(
            IntentRouter::classify_offline("help me"),
            None
        );
    }

    #[test]
    fn test_classify_offline_none() {
        assert_eq!(
            IntentRouter::classify_offline("tell me a joke"),
            None
        );
        assert_eq!(
            IntentRouter::classify_offline("what is 2 + 2"),
            None
        );
    }
}
