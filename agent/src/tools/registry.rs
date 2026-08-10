//! Tool registry — binds every built-in tool to its handler.
//!
//! `ToolRegistry` is the container that registers all built-in tools at
//! startup (`create_default_registry`) and executes them by name with
//! sandboxed paths. Handlers are `Arc<dyn Fn>` closures; the registry
//! enforces path allowlists and shares the allowlist with the filesystem
//! tools via a process-wide snapshot.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::files::files_universal;
use super::files::filesystem;
use super::hardware::device;
use super::hardware::display;
use super::hardware::hardware_universal;
use super::hardware::input;
use super::network::connectivity;
use super::network::external;
use super::system::help;
use super::system::os;
use super::system::power;
use super::system::process;
use super::system::security;
use super::system::update;
use super::{ToolDefinition, ToolHandler, ToolResult};

/// Snapshot of all registered tool definitions, captured after the default
/// registry is built. Read by `help::show_tool_list`.
static REGISTERED_TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();

/// Snapshot of the active sandbox allowlist, set by `ToolRegistry::new`.
///
/// Filesystem tools read this through `sandbox_allowed_paths()`; handlers are
/// `Arc<dyn Fn>` closures without access to the registry, so the allowlist is
/// shared via this process-wide snapshot (the app builds a single registry).
static SANDBOX_ALLOWED_PATHS: OnceLock<Vec<String>> = OnceLock::new();

/// Current sandbox allowlist (empty until a registry has been created).
fn sandbox_allowed_paths() -> &'static [String] {
    SANDBOX_ALLOWED_PATHS
        .get()
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

/// Returns the registered-tools snapshot (empty until the default registry
/// has been created at least once).
pub(crate) fn registered_tools_snapshot() -> &'static [ToolDefinition] {
    REGISTERED_TOOLS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Registry of available tools.
///
/// Tools are registered at startup and can be invoked by name.
/// All tools are sandboxed to allowed paths defined in the configuration.
pub struct ToolRegistry {
    tools: HashMap<String, RegisteredTool>,
    allowed_paths: Vec<String>,
    /// Short-TTL cache of deterministic tool results (spec 7.2): identical
    /// calls inside one agentic loop (file_list, system info, process list)
    /// hit the cache instead of re-running. Cleared on idle stage 1.
    cache: Mutex<HashMap<String, (Instant, ToolResult)>>,
}

/// Tools whose result is deterministic over a ~2 s window (spec 7.2).
const DETERMINISTIC_TOOLS: &[&str] = &["file_list", "get_system_info", "list_processes"];

/// Cache lifetime for deterministic tool results.
const TOOL_CACHE_TTL: Duration = Duration::from_secs(2);

/// Hard cap on cached entries (LRU-ish: oldest evicted first).
const TOOL_CACHE_CAP: usize = 64;

struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
}

impl ToolRegistry {
    /// Create a new empty ToolRegistry with the given allowed paths.
    pub fn new(allowed_paths: Vec<String>) -> Self {
        // Share the allowlist with filesystem tool handlers (see
        // `sandbox_allowed_paths`). First writer wins; the app builds exactly
        // one registry per process.
        let _ = SANDBOX_ALLOWED_PATHS.set(allowed_paths.clone());
        Self {
            tools: HashMap::new(),
            allowed_paths,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Register a tool with its definition and handler function.
    pub fn register_tool(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        handler: ToolHandler,
    ) {
        let name = name.into();
        self.tools.insert(
            name.clone(),
            RegisteredTool {
                definition: ToolDefinition {
                    name,
                    description: description.into(),
                    parameters,
                },
                handler,
            },
        );
    }

    /// Execute a tool by name with the given parameters.
    pub fn execute_tool(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<ToolResult> {
        let tool = self.tools.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown tool: {name}. Available: {}",
                self.list_tool_names().join(", ")
            )
        })?;

        // Deterministic tools get a 2 s TTL cache (spec 7.2).
        if DETERMINISTIC_TOOLS.contains(&name) {
            let key = format!(
                "{name}:{}",
                serde_json::to_string(&params).unwrap_or_default()
            );
            if let Some(hit) = cache_lookup(
                &mut self.cache.lock().unwrap_or_else(|e| e.into_inner()),
                &key,
                Instant::now(),
            ) {
                return Ok(hit);
            }
            let result = (tool.handler)(params)?;
            cache_store(
                &mut self.cache.lock().unwrap_or_else(|e| e.into_inner()),
                key,
                result.clone(),
                Instant::now(),
            );
            return Ok(result);
        }

        (tool.handler)(params)
    }

    /// Drop all cached tool results (idle stage 1, spec 3.2).
    pub fn clear_cache(&self) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// List all registered tool definitions.
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition.clone()).collect()
    }

    /// List just the names of registered tools.
    pub fn list_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get the allowed paths for sandboxing.
    pub fn allowed_paths(&self) -> &[String] {
        &self.allowed_paths
    }

    /// Check if a path is within allowed directories.
    pub fn is_path_allowed(&self, path: &str) -> bool {
        if self.allowed_paths.is_empty() {
            return true; // No restrictions
        }

        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => {
                // If the path doesn't exist yet, do a simple prefix check
                let expanded = shellexpand::tilde(path).to_string();
                return self
                    .allowed_paths
                    .iter()
                    .any(|allowed| expanded.starts_with(&shellexpand::tilde(allowed).to_string()));
            }
        };

        self.allowed_paths.iter().any(|allowed| {
            let allowed_canonical = std::fs::canonicalize(shellexpand::tilde(allowed).as_ref())
                .unwrap_or_else(|_| std::path::PathBuf::from(shellexpand::tilde(allowed).as_ref()));
            canonical.starts_with(&allowed_canonical)
        })
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new(vec!["~".into(), "/tmp".into(), "/etc/diad".into()])
    }
}

/// Create a default ToolRegistry with all built-in tools registered.
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::default();

    // ---- Filesystem tools ----
    registry.register_tool(
        "read_file",
        "Read the contents of a file",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        }),
        Arc::new(|params| {
            let path = params["path"].as_str().unwrap_or("");
            filesystem::read_file(path, sandbox_allowed_paths())
        }),
    );

    registry.register_tool(
        "write_file",
        "Write content to a file",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        }),
        Arc::new(|params| {
            let path = params["path"].as_str().unwrap_or("");
            let content = params["content"].as_str().unwrap_or("");
            filesystem::write_file(path, content, sandbox_allowed_paths())
        }),
    );

    registry.register_tool(
        "list_dir",
        "List the contents of a directory",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                }
            },
            "required": ["path"]
        }),
        Arc::new(|params| {
            let path = params["path"].as_str().unwrap_or(".");
            filesystem::list_dir(path, sandbox_allowed_paths())
        }),
    );

    registry.register_tool(
        "search_files",
        "Search for files matching a pattern",
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (glob or regex)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in"
                }
            },
            "required": ["pattern"]
        }),
        Arc::new(|params| {
            let pattern = params["pattern"].as_str().unwrap_or("*");
            let path = params["path"].as_str().unwrap_or(".");
            filesystem::search_files(pattern, path, sandbox_allowed_paths())
        }),
    );

    registry.register_tool(
        "chmod_file",
        "Change file permissions",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "mode": {
                    "type": "string",
                    "description": "Permission mode (octal string, e.g., '755')"
                }
            },
            "required": ["path", "mode"]
        }),
        Arc::new(|params| {
            let path = params["path"].as_str().unwrap_or("");
            let mode = params["mode"].as_str().unwrap_or("644");
            filesystem::chmod_file(path, mode, sandbox_allowed_paths())
        }),
    );

    // ---- Process tools ----
    registry.register_tool(
        "spawn_process",
        "Spawn a new process",
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command arguments"
                }
            },
            "required": ["command"]
        }),
        Arc::new(|params| {
            let command = params["command"].as_str().unwrap_or("");
            let args: Vec<String> = params["args"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            process::spawn_process(command, &args)
        }),
    );

    registry.register_tool(
        "list_processes",
        "List running processes",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| process::list_processes()),
    );

    registry.register_tool(
        "kill_process",
        "Kill a process by PID",
        serde_json::json!({
            "type": "object",
            "properties": {
                "pid": {
                    "type": "integer",
                    "description": "Process ID to kill"
                }
            },
            "required": ["pid"]
        }),
        Arc::new(|params| {
            let pid = params["pid"].as_u64().unwrap_or(0) as u32;
            process::kill_process(pid)
        }),
    );

    // ---- Network tools ----
    registry.register_tool(
        "http_get",
        "Perform an HTTP GET request",
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                }
            },
            "required": ["url"]
        }),
        Arc::new(|params| {
            let url = params["url"].as_str().unwrap_or("");
            connectivity::http_get(url)
        }),
    );

    registry.register_tool(
        "http_post",
        "Perform an HTTP POST request",
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to post to"
                },
                "body": {
                    "type": "string",
                    "description": "Request body"
                }
            },
            "required": ["url"]
        }),
        Arc::new(|params| {
            let url = params["url"].as_str().unwrap_or("");
            let body = params["body"].as_str().unwrap_or("");
            connectivity::http_post(url, body)
        }),
    );

    registry.register_tool(
        "ping_host",
        "Ping a host to check connectivity",
        serde_json::json!({
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "description": "Hostname or IP to ping"
                }
            },
            "required": ["host"]
        }),
        Arc::new(|params| {
            let host = params["host"].as_str().unwrap_or("");
            connectivity::ping_host(host)
        }),
    );

    // ---- System tools ----
    registry.register_tool(
        "get_system_info",
        "Get system information",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| os::get_system_info()),
    );

    registry.register_tool(
        "install_package",
        "Install a system package",
        serde_json::json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "Package name to install"
                }
            },
            "required": ["package"]
        }),
        Arc::new(|params| {
            let package = params["package"].as_str().unwrap_or("");
            os::install_package(package)
        }),
    );

    registry.register_tool(
        "remove_package",
        "Remove a system package",
        serde_json::json!({
            "type": "object",
            "properties": {
                "package": {
                    "type": "string",
                    "description": "Package name to remove"
                }
            },
            "required": ["package"]
        }),
        Arc::new(|params| {
            let package = params["package"].as_str().unwrap_or("");
            os::remove_package(package)
        }),
    );

    registry.register_tool(
        "manage_service",
        "Manage a systemd service",
        serde_json::json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "Service name"
                },
                "action": {
                    "type": "string",
                    "enum": ["start", "stop", "restart", "enable", "disable", "status"],
                    "description": "Action to perform"
                }
            },
            "required": ["service", "action"]
        }),
        Arc::new(|params| {
            let service = params["service"].as_str().unwrap_or("");
            let action = params["action"].as_str().unwrap_or("status");
            os::manage_service(service, action)
        }),
    );

    // ---- Hardware tools (system control: Wi-Fi, audio, brightness, power, Bluetooth) ----
    registry.register_tool(
        "wifi_list",
        "List nearby Wi-Fi networks (SSID, signal, security)",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| device::wifi_list()),
    );

    registry.register_tool(
        "wifi_connect",
        "Connect to a Wi-Fi network by SSID. If no password is provided, the user is prompted securely (password is never shown or stored).",
        serde_json::json!({
            "type": "object",
            "properties": {
                "ssid": {
                    "type": "string",
                    "description": "The SSID (network name) to connect to"
                },
                "password": {
                    "type": "string",
                    "description": "The network password. Omit to trigger a secure masked prompt."
                }
            },
            "required": ["ssid"]
        }),
        Arc::new(|params| {
            let ssid = params["ssid"].as_str().unwrap_or("");
            let password = params["password"].as_str();
            device::wifi_connect(ssid, password)
        }),
    );

    registry.register_tool(
        "wifi_status",
        "Show NetworkManager general status and device status",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| device::wifi_status()),
    );

    registry.register_tool(
        "wifi_disconnect",
        "Disconnect from the current Wi-Fi network",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| device::wifi_disconnect()),
    );

    registry.register_tool(
        "get_volume",
        "Get the current audio volume of the default sink",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| device::get_volume()),
    );

    registry.register_tool(
        "set_volume",
        "Set the audio volume of the default sink to a percentage (0-100)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "percent": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100,
                    "description": "Volume percentage (0-100)"
                }
            },
            "required": ["percent"]
        }),
        Arc::new(|params| {
            let percent = params["percent"].as_u64().unwrap_or(0).min(100) as u8;
            device::set_volume(percent)
        }),
    );

    registry.register_tool(
        "toggle_mute",
        "Toggle mute on the default audio sink",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| device::toggle_mute()),
    );

    registry.register_tool(
        "get_brightness",
        "Get the current screen brightness level",
        serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        Arc::new(|_params| device::get_brightness()),
    );

    registry.register_tool(
        "set_brightness",
        "Set the screen brightness to a percentage (0-100)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "percent": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100,
                    "description": "Brightness percentage (0-100)"
                }
            },
            "required": ["percent"]
        }),
        Arc::new(|params| {
            let percent = params["percent"].as_u64().unwrap_or(0).min(100) as u8;
            device::set_brightness(percent)
        }),
    );

    registry.register_tool(
        "power",
        "Power control: shutdown, reboot, or suspend the system. Requires explicit user confirmation in the TUI before executing.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["shutdown", "reboot", "suspend"],
                    "description": "The power action to perform"
                }
            },
            "required": ["action"]
        }),
        Arc::new(|params| {
            let action = params["action"].as_str().unwrap_or("");
            device::power(action)
        }),
    );

    registry.register_tool(
        "bluetooth",
        "Control Bluetooth: power on/off, or connect/disconnect a device by MAC address",
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["on", "off", "connect", "disconnect"],
                    "description": "Bluetooth action"
                },
                "device": {
                    "type": "string",
                    "description": "Device MAC address (required for connect/disconnect)"
                }
            },
            "required": ["action"]
        }),
        Arc::new(|params| {
            let action = params["action"].as_str().unwrap_or("");
            let device = params["device"].as_str();
            device::bluetooth(action, device)
        }),
    );

    registry.register_tool(
        "get_network_interfaces",
        "List all network interfaces with their addresses (JSON)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| device::get_network_interfaces()),
    );

    registry.register_tool(
        "connect_ethernet",
        "Connect an ethernet (wired) interface via NetworkManager",
        serde_json::json!({
            "type": "object",
            "properties": {
                "interface": {
                    "type": "string",
                    "description": "Interface name, e.g. enp0s3"
                }
            },
            "required": ["interface"]
        }),
        Arc::new(|params| {
            let interface = params["interface"].as_str().unwrap_or("");
            device::connect_ethernet(interface)
        }),
    );

    registry.register_tool(
        "set_hostname",
        "Set the system hostname",
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The new hostname"
                }
            },
            "required": ["name"]
        }),
        Arc::new(|params| {
            let name = params["name"].as_str().unwrap_or("");
            device::set_hostname(name)
        }),
    );

    // ---- Laptop input / media keys ----
    registry.register_tool(
        "keyboard_brightness_up",
        "Increase the keyboard backlight by 10%",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::keyboard_brightness_up()),
    );

    registry.register_tool(
        "keyboard_brightness_down",
        "Decrease the keyboard backlight by 10%",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::keyboard_brightness_down()),
    );

    registry.register_tool(
        "volume_up",
        "Raise the default audio sink volume by 5%",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::volume_up()),
    );

    registry.register_tool(
        "volume_down",
        "Lower the default audio sink volume by 5%",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::volume_down()),
    );

    registry.register_tool(
        "volume_mute",
        "Toggle mute on the default audio sink",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::volume_mute()),
    );

    registry.register_tool(
        "airplane_mode",
        "Toggle airplane mode: true turns the Wi-Fi radio off, false turns it on",
        serde_json::json!({
            "type": "object",
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "description": "true = airplane mode ON (Wi-Fi off)"
                }
            },
            "required": ["enabled"]
        }),
        Arc::new(|params| {
            let enabled = params["enabled"].as_bool().unwrap_or(false);
            input::airplane_mode(enabled)
        }),
    );

    registry.register_tool(
        "media_play_pause",
        "Toggle play/pause on the active media player",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::media_play_pause()),
    );

    registry.register_tool(
        "media_next",
        "Skip to the next track on the active media player",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::media_next()),
    );

    registry.register_tool(
        "media_previous",
        "Go back to the previous track on the active media player",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| input::media_previous()),
    );

    // ---- Display & multi-monitor ----
    registry.register_tool(
        "list_outputs",
        "List all display outputs and their current state",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| display::list_outputs()),
    );

    registry.register_tool(
        "set_resolution",
        "Set a display output's resolution (width x height)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "output": {
                    "type": "string",
                    "description": "Output name, e.g. HDMI-A-1"
                },
                "width": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Width in pixels"
                },
                "height": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Height in pixels"
                }
            },
            "required": ["output", "width", "height"]
        }),
        Arc::new(|params| {
            let output = params["output"].as_str().unwrap_or("");
            let width = params["width"].as_u64().unwrap_or(0) as u32;
            let height = params["height"].as_u64().unwrap_or(0) as u32;
            display::set_resolution(output, width, height)
        }),
    );

    registry.register_tool(
        "set_scale",
        "Set a display output's scale factor (e.g. 1.0, 1.25, 2.0)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "output": {
                    "type": "string",
                    "description": "Output name, e.g. eDP-1"
                },
                "factor": {
                    "type": "number",
                    "description": "Scale factor in (0, 10]"
                }
            },
            "required": ["output", "factor"]
        }),
        Arc::new(|params| {
            let output = params["output"].as_str().unwrap_or("");
            let factor = params["factor"].as_f64().unwrap_or(1.0);
            display::set_scale(output, factor)
        }),
    );

    registry.register_tool(
        "set_position",
        "Set a display output's position in the layout (pixels)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "output": {
                    "type": "string",
                    "description": "Output name, e.g. HDMI-A-1"
                },
                "x": {
                    "type": "integer",
                    "description": "X offset in pixels"
                },
                "y": {
                    "type": "integer",
                    "description": "Y offset in pixels"
                }
            },
            "required": ["output", "x", "y"]
        }),
        Arc::new(|params| {
            let output = params["output"].as_str().unwrap_or("");
            let x = params["x"].as_i64().unwrap_or(0) as i32;
            let y = params["y"].as_i64().unwrap_or(0) as i32;
            display::set_position(output, x, y)
        }),
    );

    registry.register_tool(
        "mirror_outputs",
        "Mirror the secondary output onto the primary output",
        serde_json::json!({
            "type": "object",
            "properties": {
                "primary": {
                    "type": "string",
                    "description": "Primary (source) output name"
                },
                "secondary": {
                    "type": "string",
                    "description": "Secondary (mirrored) output name"
                }
            },
            "required": ["primary", "secondary"]
        }),
        Arc::new(|params| {
            let primary = params["primary"].as_str().unwrap_or("");
            let secondary = params["secondary"].as_str().unwrap_or("");
            display::mirror_outputs(primary, secondary)
        }),
    );

    // ---- Power management ----
    registry.register_tool(
        "get_battery_status",
        "Get the battery status (state, percentage, time remaining) — laptops only",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| power::get_battery_status()),
    );

    registry.register_tool(
        "set_power_profile",
        "Set the power profile: performance, balanced, or power-saver",
        serde_json::json!({
            "type": "object",
            "properties": {
                "profile": {
                    "type": "string",
                    "enum": ["performance", "balanced", "power-saver"],
                    "description": "The power profile to activate"
                }
            },
            "required": ["profile"]
        }),
        Arc::new(|params| {
            let profile = params["profile"].as_str().unwrap_or("");
            power::set_power_profile(profile)
        }),
    );

    registry.register_tool(
        "get_power_profile",
        "Get the currently active power profile",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| power::get_power_profile()),
    );

    registry.register_tool(
        "set_sleep_timeout",
        "Set the AC (plugged-in) sleep timeout in minutes (1-1440)",
        serde_json::json!({
            "type": "object",
            "properties": {
                "minutes": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1440,
                    "description": "Minutes of inactivity before suspend"
                }
            },
            "required": ["minutes"]
        }),
        Arc::new(|params| {
            let minutes = params["minutes"].as_u64().unwrap_or(0) as u32;
            power::set_sleep_timeout(minutes)
        }),
    );

    // ---- Security ----
    registry.register_tool(
        "lock_screen",
        "Lock the screen immediately (solid black via swaylock)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| security::lock_screen()),
    );

    registry.register_tool(
        "enable_firewall",
        "Enable the DIAD nftables firewall (default-drop inbound policy)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| security::enable_firewall()),
    );

    registry.register_tool(
        "disable_firewall",
        "Disable the DIAD nftables firewall",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| security::disable_firewall()),
    );

    // ---- Updates ----
    registry.register_tool(
        "check_system_updates",
        "Check for pending system package updates (checkupdates)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| update::check_system_updates()),
    );

    registry.register_tool(
        "apply_system_updates",
        "Apply all system updates (pacman -Syu). Requires explicit user confirmation before executing.",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| update::apply_system_updates()),
    );

    registry.register_tool(
        "check_agent_updates",
        "Check for a newer DIAD Agent release (placeholder releases endpoint)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| update::check_agent_updates()),
    );

    registry.register_tool(
        "apply_agent_updates",
        "Download and install the latest DIAD Agent binary (placeholder releases endpoint)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| update::apply_agent_updates()),
    );

    // ---- Help / info ----
    registry.register_tool(
        "show_help",
        "Show the comprehensive DIAD help text (slash commands, keybindings, offline intents, tool categories)",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| help::show_help()),
    );

    registry.register_tool(
        "show_tool_list",
        "List every registered tool with its description",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| help::show_tool_list()),
    );

    registry.register_tool(
        "show_version",
        "Show the DIAD Agent version and build information",
        serde_json::json!({ "type": "object", "properties": {} }),
        Arc::new(|_params| help::show_version()),
    );

    // ---- External AI coding CLIs ----
    registry.register_tool(
        "delegate_to_agent",
        "Delegate a task to an installed local AI coding CLI (claude, codex, aider, opencode, gemini). The CLI runs non-interactively and its output streams back into the chat. Requires explicit user confirmation before spawning.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "enum": ["claude", "codex", "aider", "opencode", "gemini"],
                    "description": "The local AI coding CLI to invoke"
                },
                "task": {
                    "type": "string",
                    "description": "The task to delegate (passed as a prompt to the CLI)"
                }
            },
            "required": ["agent", "task"]
        }),
        Arc::new(|params| {
            let agent = params["agent"].as_str().unwrap_or("").to_string();
            let task = params["task"].as_str().unwrap_or("").to_string();
            external::delegate_to_agent(&agent, &task)
        }),
    );

    // ---- Universal hardware tools (11 subsections) ----
    hardware_universal::register_universal_tools(&mut registry);

    // ---- Universal file tools (6 subsections) ----
    files_universal::register_universal_file_tools(&mut registry);

    //
    // Capture the full tool inventory for help::show_tool_list.
    let _ = REGISTERED_TOOLS.set(registry.list_tools());

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::testutil::HOME_LOCK;
    use crate::tools::valid_identifier;

    #[test]
    fn test_registry_default() {
        let registry = create_default_registry();
        let tools = registry.list_tools();
        assert!(!tools.is_empty(), "Tool registry should have tools");
    }

    #[test]
    fn test_registry_list_names() {
        let registry = create_default_registry();
        let names = registry.list_tool_names();
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"get_system_info".to_string()));
    }

    #[test]
    fn test_registry_hardware_tools() {
        let registry = create_default_registry();
        let names = registry.list_tool_names();
        for tool in [
            "wifi_list",
            "wifi_connect",
            "wifi_status",
            "wifi_disconnect",
            "get_volume",
            "set_volume",
            "toggle_mute",
            "get_brightness",
            "set_brightness",
            "power",
            "bluetooth",
        ] {
            assert!(names.contains(&tool.to_string()), "missing tool: {tool}");
        }
    }

    #[test]
    fn test_registry_wifi_connect_missing_password_requests_secret() {
        let registry = create_default_registry();
        let result = registry
            .execute_tool("wifi_connect", serde_json::json!({ "ssid": "TestNet" }))
            .unwrap();
        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(err.starts_with(device::NEED_SECRET_PREFIX));
    }

    #[test]
    fn test_registry_release_tools() {
        let registry = create_default_registry();
        let names = registry.list_tool_names();
        for tool in [
            // Laptop input / media keys
            "keyboard_brightness_up",
            "keyboard_brightness_down",
            "volume_up",
            "volume_down",
            "volume_mute",
            "airplane_mode",
            "media_play_pause",
            "media_next",
            "media_previous",
            // Display & multi-monitor
            "list_outputs",
            "set_resolution",
            "set_scale",
            "set_position",
            "mirror_outputs",
            // Power management
            "get_battery_status",
            "set_power_profile",
            "get_power_profile",
            "set_sleep_timeout",
            // Security
            "lock_screen",
            "enable_firewall",
            "disable_firewall",
            // Updates
            "check_system_updates",
            "apply_system_updates",
            "check_agent_updates",
            "apply_agent_updates",
            // Help
            "show_help",
            "show_tool_list",
            "show_version",
        ] {
            assert!(names.contains(&tool.to_string()), "missing tool: {tool}");
        }
    }

    // ── Phase 7: security regression tests ──────────────────────────────

    #[test]
    fn test_valid_identifier_accepts_sane_names() {
        for s in [
            "wifi_connect",
            "set_volume",
            "media_play_pause",
            "foo@bar",
            "a.b-c_1",
            "x:+y",
        ] {
            assert!(valid_identifier(s), "should accept {s:?}");
        }
    }

    #[test]
    fn test_valid_identifier_rejects_shell_injection() {
        for s in [
            "a;b",
            "a&&b",
            "a|b",
            "a`id`",
            "$(id)",
            "${HOME}",
            "a b",
            "a\tb",
            "a\nb",
            "a\"b",
            "a'b",
            "a\\b",
            "/etc/passwd",
            "../up",
            "-x",
            "--help",
            "a, b",
            "a*b",
            "a?b",
        ] {
            assert!(!valid_identifier(s), "should reject {s:?}");
        }
    }

    #[test]
    fn test_valid_identifier_rejects_leading_dash() {
        // Leading dash would make a name parse as an option to a CLI tool.
        for s in ["-n", "--dry-run", "-", "-x", "--"] {
            assert!(!valid_identifier(s), "should reject {s:?}");
        }
    }

    #[test]
    fn test_valid_identifier_rejects_empty_and_whitespace() {
        for s in ["", "   ", "\t", "\n"] {
            assert!(!valid_identifier(s), "should reject {s:?}");
        }
    }

    #[test]
    fn test_is_path_allowed_allows_configured_roots() {
        // HOME is process-global and shared with tool tests that point it at
        // temp dirs; serialize access (see tools::testutil) so `~` expansion
        // sees a stable value for the whole assertion.
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let registry = create_default_registry();
        // Default allowlist is unrestricted (empty = everything); when roots
        // are set, paths under them are allowed.
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
        assert!(registry.is_path_allowed(&format!("{home}/documents/report.pdf")));
        assert!(registry.is_path_allowed(&format!("{home}/.diad/config.toml")));
    }

    #[test]
    fn test_is_path_allowed_rejects_traversal_escape() {
        // Canonicalization must prevent ../ escapes from a root.
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/diad".into());
        // With the default unrestricted registry, traversal is allowed by
        // design; this test asserts that when an allowlist root is set the
        // check is enforced. Construct a registry with an explicit root.
        let restricted = ToolRegistry::new(vec![home.clone(), "/tmp".to_string()]);
        assert!(restricted.is_path_allowed(&format!("{home}/ok/file.txt")));
        assert!(restricted.is_path_allowed("/tmp/scratch.dat"));
        // Traversal above a root must be rejected even if the canonical
        // target exists (e.g. /home/user/../../etc → /etc).
        assert!(!restricted.is_path_allowed("/etc/passwd"));
    }

    #[test]
    fn test_registry_has_no_sh_or_sudo_tools() {
        // Phase 7: the tool registry must never expose a raw shell or
        // privilege-elevation tool. Everything privileged goes through the
        // sudoers allowlist + diad-sysfs-write helper.
        let registry = create_default_registry();
        let names = registry.list_tool_names();
        for banned in ["run_shell", "sh", "bash", "sudo", "run_root", "shell"] {
            assert!(
                !names.contains(&banned.to_string()),
                "banned tool present: {banned}"
            );
        }
    }
}

/// TTL lookup: returns the cached result only while fresh, evicting stale
/// entries as a side effect. Pure over the passed map + clock.
fn cache_lookup(
    cache: &mut HashMap<String, (Instant, ToolResult)>,
    key: &str,
    now: Instant,
) -> Option<ToolResult> {
    match cache.get(key) {
        Some((stored_at, result)) if now.duration_since(*stored_at) < TOOL_CACHE_TTL => {
            Some(result.clone())
        }
        Some(_) => {
            cache.remove(key);
            None
        }
        None => None,
    }
}

/// Insert with a size cap: when full, evict the oldest entry first.
fn cache_store(
    cache: &mut HashMap<String, (Instant, ToolResult)>,
    key: String,
    result: ToolResult,
    now: Instant,
) {
    if cache.len() >= TOOL_CACHE_CAP {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(key, (now, result));
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn ok_result(text: &str) -> ToolResult {
        ToolResult::ok(text.to_string())
    }

    #[test]
    fn cache_hits_within_ttl_and_expires() {
        let mut cache = HashMap::new();
        let t0 = Instant::now();
        cache_store(&mut cache, "k".into(), ok_result("v"), t0);

        assert!(cache_lookup(&mut cache, "k", t0).is_some());
        assert!(cache_lookup(
            &mut cache,
            "k",
            t0 + TOOL_CACHE_TTL - Duration::from_millis(1)
        )
        .is_some());
        assert!(cache_lookup(&mut cache, "k", t0 + TOOL_CACHE_TTL).is_none());
        assert!(cache.is_empty(), "stale entry must be evicted");
    }

    #[test]
    fn cache_cap_evicts_oldest() {
        let mut cache = HashMap::new();
        let t0 = Instant::now();
        for i in 0..=TOOL_CACHE_CAP {
            cache_store(
                &mut cache,
                format!("k{i}"),
                ok_result("v"),
                t0 + Duration::from_millis(i as u64),
            );
        }
        assert_eq!(cache.len(), TOOL_CACHE_CAP);
        assert!(!cache.contains_key("k0"), "oldest entry evicted");
        assert!(cache.contains_key(&format!("k{}", TOOL_CACHE_CAP)));
    }

    #[test]
    fn registry_clear_cache_empties_store() {
        let registry = ToolRegistry::new(vec![]);
        {
            let mut cache = registry.cache.lock().unwrap();
            cache_store(&mut cache, "k".into(), ok_result("v"), Instant::now());
        }
        registry.clear_cache();
        assert!(registry.cache.lock().unwrap().is_empty());
    }
}
