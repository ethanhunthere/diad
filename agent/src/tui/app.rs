//! TUI application state and key handling.
//!
//! Owns the mode stack (`Input` / `Tree` / `Code` / `Sys` / `Model`), the
//! input buffer, and forwards every event to the daemon client.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::settings::DiadConfig;
use crate::providers::ProviderEntry;

/// The current mode of the DIAD Agent TUI.
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Primary chat interface with the LLM
    Chat,
    /// File tree browser
    FileTree,
    /// Code viewer/editor
    CodeView,
    /// System status dashboard
    SystemStatus,
    /// API key input (masked)
    Connect,
    /// Reusable masked secret input (Wi-Fi passwords, etc.)
    SecurePrompt,
    /// Provider picker (F6 / `/model`) — select provider then model
    ProviderPicker,
    /// Model picker — arrow keys select the active model
    ModelPicker,
}

impl std::fmt::Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppMode::Chat => write!(f, "Chat"),
            AppMode::FileTree => write!(f, "File Tree"),
            AppMode::CodeView => write!(f, "Code View"),
            AppMode::SystemStatus => write!(f, "System Status"),
            AppMode::Connect => write!(f, "Connect"),
            AppMode::SecurePrompt => write!(f, "Secure Prompt"),
            AppMode::ProviderPicker => write!(f, "Provider Picker"),
            AppMode::ModelPicker => write!(f, "Model Picker"),
        }
    }
}

/// A reusable masked (hidden) input prompt — e.g. Wi-Fi passwords.
///
/// The value the user types is delivered to `reply` and immediately cleared.
/// It is NEVER logged, stored in messages, or sent to the LLM.
pub struct SecretPrompt {
    /// Prompt label shown to the user (e.g. "Enter password for MyNetwork (hidden):")
    pub label: String,
    /// Channel used to deliver the entered value back to the orchestrator.
    pub reply: UnboundedSender<String>,
    /// Current masked input buffer (displayed as • bullets).
    pub buffer: String,
}

/// A yes/no confirmation prompt — e.g. before `power` (shutdown/reboot/suspend).
pub struct ConfirmationPrompt {
    /// Prompt label shown to the user (e.g. "Reboot system? [y/N]")
    pub label: String,
    /// Channel used to deliver the boolean answer back to the orchestrator.
    pub reply: UnboundedSender<bool>,
}

/// Multi-step offline flows (handled without a provider).
#[derive(Debug, Clone, PartialEq)]
pub enum OfflineFlow {
    /// Wi-Fi networks were listed; waiting for the user to type an SSID in
    /// the input bar. The SSID is captured in `pending_wifi_ssid`.
    WifiSelect,
}

/// The main application state.
pub struct App {
    /// User input buffer
    pub input_buffer: String,
    /// Cursor position within the input buffer
    pub cursor_position: usize,
    /// Conversation / log messages
    pub messages: Vec<String>,
    /// Current UI mode
    pub mode: AppMode,
    /// Currently selected LLM provider
    pub provider: String,
    /// Is a streaming response in progress?
    pub streaming: bool,
    /// Connection status indicator
    pub connected: bool,
    /// Application configuration
    pub config: DiadConfig,
    /// Scroll position in the message list
    pub scroll_offset: usize,
    /// Current working directory
    pub cwd: String,
    /// System info cache
    pub system_info: String,
    /// The validated Anthropic API key (never logged)
    pub api_key: Option<String>,
    /// Masked input for connect mode
    pub connect_input: String,
    /// Active secure (masked) prompt, if any
    pub secret_prompt: Option<SecretPrompt>,
    /// Active confirmation prompt, if any
    pub confirmation_prompt: Option<ConfirmationPrompt>,
    /// Active offline multi-step flow, if any
    pub offline_flow: Option<OfflineFlow>,
    /// Pending offline Wi-Fi SSID typed by the user in the normal input bar
    pub pending_wifi_ssid: Option<String>,
    /// Accumulated streaming response text
    pub stream_buffer: String,
    /// Active model for the current provider (shown in the top bar pill)
    pub model: String,
    /// Provider picker: entries (populated by the event loop from the factory)
    pub provider_entries: Vec<ProviderEntry>,
    /// Provider picker: current selection index
    pub picker_index: usize,
    /// Model picker: available models (populated by the event loop)
    pub model_list: Vec<String>,
    /// Model picker: current selection index
    pub model_index: usize,
    /// Set when the user pressed Enter on a provider in the picker; the event
    /// loop resolves it (connect if a key is needed, else switch directly).
    pub provider_selected: Option<String>,
    /// Set when the selected provider needs a key; the event loop shows a
    /// SecurePrompt, stores the key in the vault slot, then switches.
    pub pending_connect_provider: Option<String>,
    /// Set by F6 / `/model`; the event loop refreshes `provider_entries`.
    pub picker_requested: bool,
    /// Provider targeted by the Connect view (`/connect <name>` or F5).
    pub connect_target: String,
    /// Set by `/set <key> <value>` (THE LAW F-15 TUI parity); the event loop
    /// forwards it to the daemon via `config_set`.
    pub pending_config_set: Option<(String, serde_json::Value)>,
}

impl App {
    /// Create a new App with default state.
    pub fn new(provider: String, config: DiadConfig) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "/".into());
        let connect_target = provider.clone();

        Self {
            input_buffer: String::new(),
            cursor_position: 0,
            messages: vec![
                "Welcome to DIAD Agent.".into(),
                format!("Provider: {provider}"),
                "Type your intent or press Ctrl+D to quit. Use F1-F4 to switch views.".into(),
                "Press F6 or type /model to pick a provider & model.".into(),
            ],
            mode: AppMode::Chat,
            provider,
            streaming: false,
            connected: false,
            config,
            scroll_offset: 0,
            cwd,
            system_info: String::new(),
            api_key: None,
            connect_input: String::new(),
            secret_prompt: None,
            confirmation_prompt: None,
            offline_flow: None,
            pending_wifi_ssid: None,
            stream_buffer: String::new(),
            model: String::new(),
            provider_entries: Vec::new(),
            picker_index: 0,
            model_list: Vec::new(),
            model_index: 0,
            provider_selected: None,
            pending_connect_provider: None,
            picker_requested: false,
            connect_target,
            pending_config_set: None,
        }
    }

    /// Handle a key event, mutating app state accordingly.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.mode {
            AppMode::Chat => self.handle_chat_key(key_event),
            AppMode::FileTree => self.handle_filetree_key(key_event),
            AppMode::CodeView => self.handle_codeview_key(key_event),
            AppMode::SystemStatus => self.handle_system_key(key_event),
            AppMode::Connect => self.handle_connect_key(key_event),
            AppMode::SecurePrompt => self.handle_secure_prompt_key(key_event),
            AppMode::ProviderPicker => self.handle_picker_key(key_event),
            AppMode::ModelPicker => self.handle_model_key(key_event),
        }
    }

    /// Centralized configurable keybinding resolution.
    ///
    /// Every mode-switch / action key first checks `config.keybindings`
    /// (`action -> key spec`), falling back to the built-in defaults:
    ///
    /// | action           | default            |
    /// |------------------|--------------------|
    /// | `chat`           | F1                 |
    /// | `file_tree`      | F2                 |
    /// | `code_view`      | F3                 |
    /// | `system_status`  | F4                 |
    /// | `connect`        | F5                 |
    /// | `provider_picker`| F6                 |
    /// | `clear_chat`     | Ctrl+L             |
    ///
    /// Key specs are case-insensitive: `"f2"`, `"ctrl+l"`, `"alt+enter"`,
    /// `"esc"`, `"left"`, `"a"`. An invalid spec silently falls back to the
    /// default (the TUI must never be locked out of a mode).
    fn binding_is(&self, key_event: &KeyEvent, action: &str) -> bool {
        let default = match action {
            "chat" => KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()),
            "file_tree" => KeyEvent::new(KeyCode::F(2), KeyModifiers::empty()),
            "code_view" => KeyEvent::new(KeyCode::F(3), KeyModifiers::empty()),
            "system_status" => KeyEvent::new(KeyCode::F(4), KeyModifiers::empty()),
            "connect" => KeyEvent::new(KeyCode::F(5), KeyModifiers::empty()),
            "provider_picker" => KeyEvent::new(KeyCode::F(6), KeyModifiers::empty()),
            "clear_chat" => KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            _ => return false,
        };
        let binding = self
            .config
            .keybindings
            .get(action)
            .and_then(|spec| parse_key_spec(spec))
            .unwrap_or(default);
        &binding == key_event
    }

    /// If `key_event` switches TUI mode, return the target mode.
    fn mode_switch(&self, key_event: &KeyEvent) -> Option<AppMode> {
        for (action, mode) in [
            ("chat", AppMode::Chat),
            ("file_tree", AppMode::FileTree),
            ("code_view", AppMode::CodeView),
            ("system_status", AppMode::SystemStatus),
            ("connect", AppMode::Connect),
            ("provider_picker", AppMode::ProviderPicker),
        ] {
            if self.binding_is(key_event, action) {
                return Some(mode);
            }
        }
        None
    }

    fn handle_chat_key(&mut self, key_event: KeyEvent) {
        // Configurable: Ctrl+L clears messages.
        if self.binding_is(&key_event, "clear_chat") {
            self.messages.clear();
            return;
        }

        // Configurable mode switches (defaults F1-F6).
        if let Some(mode) = self.mode_switch(&key_event) {
            match mode {
                AppMode::Connect => {
                    self.connect_target = self.provider.clone();
                    self.connect_input.clear();
                    self.mode = AppMode::Connect;
                }
                AppMode::ProviderPicker => self.open_provider_picker(),
                m => self.mode = m,
            }
            return;
        }

        match key_event.code {
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.input_buffer.len() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.input_buffer.len();
            }
            KeyCode::Enter => {
                self.submit_input();
            }
            _ => {}
        }
    }

    /// Provider picker (F6 / `/model`): navigate with ↑/↓, select with Enter,
    /// close with Esc. Selecting a provider is resolved by the event loop
    /// (`provider_selected`), which opens the model picker afterwards.
    fn handle_picker_key(&mut self, key_event: KeyEvent) {
        if let Some(mode) = self.mode_switch(&key_event) {
            self.mode = mode;
            return;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.picker_index > 0 {
                    self.picker_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.picker_index + 1 < self.provider_entries.len() {
                    self.picker_index += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(entry) = self.provider_entries.get(self.picker_index) {
                    self.provider_selected = Some(entry.name.clone());
                }
            }
            KeyCode::Esc => self.mode = AppMode::Chat,
            _ => {}
        }
    }

    /// Model picker: arrow keys select the model, Enter activates it and
    /// returns to Chat. Esc aborts (keeping the previous model).
    fn handle_model_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.model_index > 0 {
                    self.model_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.model_index + 1 < self.model_list.len() {
                    self.model_index += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(model) = self.model_list.get(self.model_index) {
                    self.model = model.clone();
                    self.messages.push(format!(
                        "[system] Active model: {} · {}",
                        self.provider, self.model
                    ));
                    self.mode = AppMode::Chat;
                }
            }
            KeyCode::Esc => self.mode = AppMode::Chat,
            _ => {}
        }
    }

    /// Open the provider picker and request a refresh of its entries.
    pub fn open_provider_picker(&mut self) {
        self.picker_requested = true;
        self.picker_index = self
            .provider_entries
            .iter()
            .position(|e| e.name == self.provider)
            .unwrap_or(0);
        self.mode = AppMode::ProviderPicker;
    }

    fn handle_filetree_key(&mut self, key_event: KeyEvent) {
        if let Some(mode) = self.mode_switch(&key_event) {
            self.mode = mode;
            return;
        }
        match key_event.code {
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Enter => {
                self.submit_input();
            }
            _ => {}
        }
    }

    fn handle_codeview_key(&mut self, key_event: KeyEvent) {
        if let Some(mode) = self.mode_switch(&key_event) {
            self.mode = mode;
            return;
        }
        match key_event.code {
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Enter => {
                self.submit_input();
            }
            _ => {}
        }
    }

    fn handle_system_key(&mut self, key_event: KeyEvent) {
        if let Some(mode) = self.mode_switch(&key_event) {
            self.mode = mode;
            return;
        }
        match key_event.code {
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {}
            _ => {}
        }
    }

    /// Submit the current input buffer as a user message.
    pub fn submit_input(&mut self) {
        let input = self.input_buffer.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Handle /connect [provider] — connect to any provider by name.
        if let Some(rest) = input.strip_prefix("/connect") {
            let target = rest.trim();
            self.connect_target = if target.is_empty() {
                self.provider.clone()
            } else {
                target.to_string()
            };
            self.connect_input.clear();
            self.input_buffer.clear();
            self.cursor_position = 0;
            self.mode = AppMode::Connect;
            return;
        }

        // Handle /model — open the provider & model picker.
        if input == "/model" {
            self.open_provider_picker();
            self.input_buffer.clear();
            self.cursor_position = 0;
            return;
        }

        // Handle /settings — THE LAW F-15 readout: echo current config values.
        if input == "/settings" {
            let cfg = &self.config;
            self.messages.push(format!(
                "[system] settings — provider: {} · model: {}",
                self.provider, self.model
            ));
            self.messages.push(format!(
                "[system]   theme: {} · confirm_destructive: {} · stream_tokens: {}",
                cfg.theme, cfg.confirm_destructive, cfg.stream_tokens
            ));
            self.messages.push(format!(
                "[system]   max_history: {} · auto_save_interval: {}",
                cfg.max_history, cfg.auto_save_interval
            ));
            self.messages.push(
                "[system] use `/set <key> <value>` to change them".into(),
            );
            self.input_buffer.clear();
            self.cursor_position = 0;
            return;
        }

        // Handle /set <key> <value> — THE LAW F-15 TUI parity. Values are
        // parsed strictly (bool / u64 / string) and forwarded to the daemon.
        if let Some(rest) = input.strip_prefix("/set ") {
            let mut parts = rest.splitn(2, ' ');
            let key = parts.next().unwrap_or("").trim();
            let raw = parts.next().unwrap_or("").trim();
            if key.is_empty() || raw.is_empty() {
                self.messages
                    .push("[system] usage: /set <key> <value>".into());
                self.messages.push(
                    "[system] keys: confirm_destructive, stream_tokens, \
                     max_history, auto_save_interval, perf_tier, waybar_mode, \
                     keybinding {from to}".into(),
                );
            } else {
                let value: serde_json::Value = if raw == "true" {
                    serde_json::json!(true)
                } else if raw == "false" {
                    serde_json::json!(false)
                } else if let Ok(n) = raw.parse::<u64>() {
                    serde_json::json!(n)
                } else if key == "keybinding" {
                    // Format: keybinding <from> <to>
                    let mut k = rest.splitn(3, ' ');
                    let _from = k.next();
                    let from = k.next().unwrap_or("").trim();
                    let to = k.next().unwrap_or("").trim();
                    if from.is_empty() || to.is_empty() {
                        self.messages
                            .push("[system] keybinding usage: /set keybinding <from> <to>".into());
                        self.input_buffer.clear();
                        self.cursor_position = 0;
                        return;
                    }
                    serde_json::json!({ "from": from, "to": to })
                } else {
                    serde_json::json!(raw)
                };
                self.pending_config_set = Some((key.to_string(), value));
                self.messages.push(format!("[system] setting {key}…"));
            }
            self.input_buffer.clear();
            self.cursor_position = 0;
            return;
        }

        // Offline WifiSelect flow: the next Enter provides the SSID to
        // connect to (stored in `pending_wifi_ssid`, NOT sent to any LLM).
        if self.offline_flow == Some(OfflineFlow::WifiSelect) {
            self.pending_wifi_ssid = Some(input.clone());
            self.messages.push(format!("[You] {input}"));
            self.input_buffer.clear();
            self.cursor_position = 0;
            self.offline_flow = None;
            return;
        }

        self.messages.push(format!("[You] {input}"));
        self.input_buffer.clear();
        self.cursor_position = 0;

        // Mark streaming — the event loop will spawn the actual API call
        self.streaming = true;
        self.stream_buffer.clear();
    }

    /// Handle key events while in Connect mode (masked API key input).
    fn handle_connect_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char(c) => {
                self.connect_input.push(c);
            }
            KeyCode::Backspace => {
                self.connect_input.pop();
            }
            KeyCode::Enter => {
                let key = self.connect_input.trim().to_string();
                if !key.is_empty() {
                    // Store the key temporarily — validation happens in the event loop
                    self.api_key = Some(key.clone());
                    self.messages.push("[system] Validating API key...".into());
                }
                self.connect_input.clear();
                self.mode = AppMode::Chat;
            }
            KeyCode::Esc => {
                self.connect_input.clear();
                self.mode = AppMode::Chat;
            }
            _ => {}
        }
    }

    /// Handle key events while in SecurePrompt mode (masked secret input).
    ///
    /// The typed value is delivered ONLY through the prompt's reply channel —
    /// it is never stored in `messages`, never logged, and never sent to the
    /// LLM. An empty string is delivered when the user presses Esc (cancel).
    fn handle_secure_prompt_key(&mut self, key_event: KeyEvent) {
        // Confirmation prompts are answered with y / n (or Esc = no).
        if self.confirmation_prompt.is_some() {
            match key_event.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.resolve_confirmation(true),
                KeyCode::Char('n') | KeyCode::Char('N') => self.resolve_confirmation(false),
                KeyCode::Esc => self.resolve_confirmation(false),
                _ => {}
            }
            return;
        }

        let Some(prompt) = &mut self.secret_prompt else {
            self.mode = AppMode::Chat;
            return;
        };

        match key_event.code {
            KeyCode::Char(c) => {
                prompt.buffer.push(c);
            }
            KeyCode::Backspace => {
                prompt.buffer.pop();
            }
            KeyCode::Enter => {
                let value = std::mem::take(&mut prompt.buffer);
                // Send the value to the awaiting orchestrator task
                let _ = prompt.reply.send(value);
                self.secret_prompt = None;
                self.mode = AppMode::Chat;
            }
            KeyCode::Esc => {
                prompt.buffer.clear();
                let _ = prompt.reply.send(String::new());
                self.secret_prompt = None;
                self.mode = AppMode::Chat;
            }
            _ => {}
        }
    }

    /// Enter SecurePrompt mode with the given label.
    ///
    /// The `reply` channel delivers the entered value (empty string = cancel).
    pub fn request_secret(&mut self, label: String, reply: UnboundedSender<String>) {
        self.secret_prompt = Some(SecretPrompt {
            label,
            reply,
            buffer: String::new(),
        });
        self.mode = AppMode::SecurePrompt;
    }

    /// Enter ConfirmationPrompt mode with the given label.
    ///
    /// The `reply` channel delivers the user's yes/no answer.
    pub fn request_confirmation(&mut self, label: String, reply: UnboundedSender<bool>) {
        self.confirmation_prompt = Some(ConfirmationPrompt { label, reply });
        self.mode = AppMode::SecurePrompt;
    }

    /// Resolve the active confirmation prompt from the chat input bar.
    /// In SecurePrompt mode the confirmation is answered with 'y'/'n' keys.
    pub fn resolve_confirmation(&mut self, accepted: bool) {
        if let Some(prompt) = self.confirmation_prompt.take() {
            let _ = prompt.reply.send(accepted);
        }
        self.mode = AppMode::Chat;
    }

    /// Called by the event loop when the offline flow asked the user to type
    /// an SSID into the normal input bar. Stores it and returns it.
    pub fn complete_pending_wifi(&mut self) -> Option<String> {
        self.pending_wifi_ssid.take()
    }

    /// Move the cursor forward one position.
    pub fn advance_cursor(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            self.cursor_position += 1;
        }
    }

    /// Move the cursor back one position.
    pub fn retreat_cursor(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }
}

/// Parse a human keybinding spec like `"f2"`, `"ctrl+l"`, `"alt+enter"`,
/// `"esc"`, `"left"`, `"a"` into a [`KeyEvent`]. Case-insensitive. Returns
/// `None` for unknown specs so callers can fall back to defaults.
fn parse_key_spec(spec: &str) -> Option<KeyEvent> {
    let spec = spec.trim().to_lowercase();
    if spec.is_empty() {
        return None;
    }
    let mut modifiers = KeyModifiers::empty();
    let key_part = match spec.split_once('+') {
        Some((mods, key)) => {
            for part in mods.split('+') {
                match part {
                    "ctrl" | "control" => modifiers.insert(KeyModifiers::CONTROL),
                    "alt" | "meta" => modifiers.insert(KeyModifiers::ALT),
                    "shift" => modifiers.insert(KeyModifiers::SHIFT),
                    _ => return None,
                }
            }
            key
        }
        None => spec.as_str(),
    };
    let code = match key_part {
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next().unwrap()),
        _ => return None,
    };
    Some(KeyEvent::new(code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::DiadConfig;

    #[test]
    fn test_parse_key_spec_basic() {
        assert_eq!(
            parse_key_spec("f2"),
            Some(KeyEvent::new(KeyCode::F(2), KeyModifiers::empty()))
        );
        assert_eq!(
            parse_key_spec("F4"),
            Some(KeyEvent::new(KeyCode::F(4), KeyModifiers::empty()))
        );
        assert_eq!(
            parse_key_spec("ctrl+l"),
            Some(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL))
        );
        assert_eq!(
            parse_key_spec("Alt+Enter"),
            Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT))
        );
        assert_eq!(
            parse_key_spec("esc"),
            Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()))
        );
        assert_eq!(
            parse_key_spec("a"),
            Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()))
        );
    }

    #[test]
    fn test_parse_key_spec_rejects_garbage() {
        for spec in [
            "",
            "  ",
            "f13",
            "ctrl+f14",
            "bogus+key",
            "ctrl+",
            "++x",
            "ctrl+alt+shift+meta+x",
        ] {
            assert_eq!(parse_key_spec(spec), None, "should reject {spec:?}");
        }
    }

    #[test]
    fn test_default_keybindings_fall_back() {
        let app = App::new("anthropic".into(), DiadConfig::default());
        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::empty());
        assert!(app.binding_is(&f2, "file_tree"));
        assert!(app.binding_is(&f2, "file_tree"));
        let ctrl_l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert!(app.binding_is(&ctrl_l, "clear_chat"));
        // Defaults never overlap across actions.
        for (a, k) in [
            ("chat", KeyCode::F(1)),
            ("file_tree", KeyCode::F(2)),
            ("code_view", KeyCode::F(3)),
            ("system_status", KeyCode::F(4)),
            ("connect", KeyCode::F(5)),
            ("provider_picker", KeyCode::F(6)),
        ] {
            assert!(
                app.binding_is(&KeyEvent::new(k, KeyModifiers::empty()), a),
                "{a}"
            );
        }
    }

    #[test]
    fn test_custom_keybindings_override_defaults() {
        let mut config = DiadConfig::default();
        config
            .keybindings
            .insert("file_tree".into(), "ctrl+t".into());
        config
            .keybindings
            .insert("clear_chat".into(), "ctrl+x".into());
        let app = App::new("anthropic".into(), config);
        let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert!(app.binding_is(&ctrl_t, "file_tree"));
        // The old default F2 no longer opens the file tree.
        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::empty());
        assert!(!app.binding_is(&f2, "file_tree"));
        let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(app.binding_is(&ctrl_x, "clear_chat"));
    }

    #[test]
    fn test_invalid_custom_binding_falls_back_to_default() {
        let mut config = DiadConfig::default();
        config
            .keybindings
            .insert("file_tree".into(), "not-a-key".into());
        let app = App::new("anthropic".into(), config);
        // Invalid spec must not lock the user out — F2 still works.
        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::empty());
        assert!(app.binding_is(&f2, "file_tree"));
    }
}
