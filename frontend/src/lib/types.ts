// ---------------------------------------------------------------------------
// DIAD AGENT — WebSocket protocol types
// Mirrors agent/src/core/protocol.rs exactly (version v1, snake_case tags).
//
// Wire contract (guarded by the Rust `test_all_*_msg_variants_roundtrip`
// tests in protocol.rs):
// - Client → daemon: FLAT envelope — { "v": 1, "type": "<snake_case>", ... }
//   (the Rust `Envelope` flattens the message into the top level).
// - Daemon → client: bare ServerMsg — { "type": "<snake_case>", ... }
//   (no v/msg wrapper).
// ---------------------------------------------------------------------------

export const PROTOCOL_VERSION = 1;

// ---- Client → daemon -------------------------------------------------------

export type ClientMsg =
  | { type: "chat_send"; id: string; text: string; images?: string[] }
  | { type: "tool_approve"; id: string }
  | { type: "tool_reject"; id: string }
  | { type: "edit_approve"; id: string }
  | { type: "edit_reject"; id: string }
  | { type: "spawn_tui" }
  | { type: "secret_submit"; id: string; value: string }
  | { type: "secret_cancel"; id: string }
  | { type: "file_read"; id: string; path: string }
  | { type: "file_write"; id: string; path: string; content: string }
  | { type: "file_list"; id: string; path: string }
  | { type: "term_input"; data: string }
  | { type: "term_resize"; cols: number; rows: number }
  | { type: "provider_switch"; name: string }
  | { type: "model_switch"; model: string }
  | { type: "provider_connect"; id: string; name: string; key: string }
  | { type: "agent_delegate"; id: string; agent: string; task: string }
  | { type: "config_set"; key: string; value: unknown }
  | { type: "providers_list"; id: string }
  | { type: "stats_get"; id: string }
  | { type: "models_list"; id: string; provider: string }
  | { type: "agents_list"; id: string }
  // A7 parity: freedesktop trash (native, spec v1.0).
  | { type: "trash_list"; id: string }
  | { type: "trash_move"; id: string; path: string }
  | { type: "trash_restore"; id: string; name: string }
  | { type: "trash_empty"; id: string }
  // A7 parity: clipboard history (cliphist).
  | { type: "clipboard_list"; id: string }
  | { type: "clipboard_pick"; id: string; index: number }
  | { type: "clipboard_clear"; id: string }
  // MASTER FIX S2: set the login password (first-run wizard STEP 0).
  | { type: "set_user_password"; id: string; value: string };

/** Flat client envelope: `{ v: 1 }` + the message fields at the top level. */
export type ClientEnvelope = { v: number } & ClientMsg;

// ---- Daemon → client -------------------------------------------------------

export interface ProviderEntryDto {
  name: string;
  needs_key: boolean;
  has_key: boolean;
}

export interface FileEntryDto {
  name: string;
  kind: "file" | "dir" | "symlink" | string;
  size: number;
  git: string | null;
}

export type ServerMsg =
  | { type: "token"; id: string; text: string }
  | {
      type: "tool_use";
      id: string;
      call_id: string;
      name: string;
      args: unknown;
      confirm_required: boolean;
    }
  | {
      type: "tool_result";
      id: string;
      call_id: string;
      success: boolean;
      output: string;
      status_line: string;
    }
  | { type: "edit_diff"; id: string; path: string; diff: string }
  | {
      type: "status";
      provider: string;
      model: string;
      connected: boolean;
      streaming: boolean;
      daemon_version: string;
      /** THE LAW F-15: live config snapshot (theme, confirm_destructive, ...). */
      config?: {
        theme?: string;
        default_provider?: string;
        confirm_destructive?: boolean;
        stream_tokens?: boolean;
        max_history?: number;
        auto_save_interval?: number;
        /** A7 parity: mako do-not-disturb (false = notifications on). */
        dnd_enabled?: boolean;
        /** A7 parity: gammastep temperature in Kelvin (0 = off). */
        night_light_temp?: number;
      };
    }
  | { type: "providers"; id: string; entries: ProviderEntryDto[] }
  | {
      type: "models";
      id: string;
      provider: string;
      models: string[];
      current: string;
    }
  | {
      type: "stats";
      id: string;
      tokens: number;
      latency_ms: number;
      battery: string;
      wifi: string;
      cwd: string;
      idle_stage: string;
      rss_mb: number;
      perf_tier: string;
    }
  | { type: "term_output"; data: string }
  | { type: "secret_request"; id: string; label: string }
  | { type: "confirm_request"; id: string; label: string }
  | { type: "stream_done"; id: string }
  | { type: "system"; message: string }
  | { type: "error"; id: string | null; message: string }
  | { type: "agents"; id: string; agents: string[] }
  | {
      type: "connected";
      v: number;
      provider: string;
      model: string;
      connected: boolean;
      streaming: boolean;
      daemon_version: string;
      /** THE LAW F-15: live config snapshot (same shape as status.config). */
      config?: {
        theme?: string;
        default_provider?: string;
        confirm_destructive?: boolean;
        stream_tokens?: boolean;
        max_history?: number;
        auto_save_interval?: number;
        /** A7 parity: mako do-not-disturb (false = notifications on). */
        dnd_enabled?: boolean;
        /** A7 parity: gammastep temperature in Kelvin (0 = off). */
        night_light_temp?: number;
      };
    }
  | { type: "file_content"; id: string; path: string; content: string }
  | { type: "files"; id: string; path: string; entries: FileEntryDto[] }
  | { type: "file_written"; id: string; path: string }
  // A7 parity: freedesktop trash.
  | { type: "trash_list"; id: string; items: TrashEntryDto[] }
  | { type: "trash_moved"; id: string }
  | { type: "trash_restored"; id: string }
  | { type: "trash_emptied"; id: string }
  // A7 parity: clipboard history (cliphist).
  | { type: "clipboard_list"; id: string; items: ClipboardEntryDto[] }
  | { type: "clipboard_picked"; id: string }
  | { type: "clipboard_cleared"; id: string };

/** A7 parity: one entry in the freedesktop trash. */
export interface TrashEntryDto {
  name: string;
  original_path: string;
  deleted_at: string;
  size: number;
}

/** A7 parity: one clipboard-history entry (cliphist). */
export interface ClipboardEntryDto {
  index: number;
  preview: string;
}

// ---- Frontend domain types --------------------------------------------------

export type ViewId =
  | "chat"
  | "explorer"
  | "editor"
  | "terminal"
  | "agents"
  | "settings"
  | "git"
  | "debug"
  | "preview"
  | "projects"
  // A7 parity views.
  | "clipboard"
  | "trash";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  images?: string[];
  streaming?: boolean;
  /** Per-message tool call cards, in order. */
  tools: ToolCallCard[];
  /** Per-message edit diff cards, in order. */
  diffs: EditDiffCard[];
  timestamp: number;
}

export interface ToolCallCard {
  /** id echoed back in tool.approve/tool.reject. */
  requestId: string;
  callId: string;
  name: string;
  args: unknown;
  confirmRequired: boolean;
  status: "pending" | "approved" | "rejected" | "done";
  resultOutput?: string;
  resultStatus?: string;
  resultSuccess?: boolean;
  collapsed?: boolean;
}

export interface EditDiffCard {
  requestId: string;
  path: string;
  diff: string;
  status: "pending" | "accepted" | "rejected";
}

export interface Toast {
  id: string;
  kind: "info" | "success" | "warn" | "error";
  title: string;
  message?: string;
  /** Auto-dismiss delay in ms (0 = sticky). */
  ttl: number;
}

export interface OpenFile {
  path: string;
  content: string;
  dirty: boolean;
}

export interface SecretPrompt {
  id: string;
  label: string;
}

export interface ConfirmPrompt {
  id: string;
  label: string;
}

export interface SettingsState {
  theme: string;
  defaultProvider: string;
  defaultModel: string;
  storagePath: string;
  firewall: boolean;
  /** UI font scale multiplier 0.8–2.0 (spec 2.9), persisted, no restart. */
  fontScale: number;
  /** Touch/layout density override (spec 2.7). */
  density: "auto" | "compact" | "comfortable";
  /** THE LAW F-15: daemon-held preferences (reflected on open from Status). */
  confirmDestructive: boolean;
  streamTokens: boolean;
  maxHistory: number;
  autoSaveInterval: number;
  /** A7 parity: mako do-not-disturb + gammastep night light (0 = off). */
  dndEnabled: boolean;
  nightLightTemp: number;
  loaded: boolean;
}

export type OnboardingStep =
  // MASTER FIX S2: the login screen is mandatory now, so the wizard opens
  // with a "change the default password" step before anything else.
  | { kind: "password" }
  | { kind: "welcome" }
  | { kind: "provider" }
  | { kind: "wifi" }
  | { kind: "done" };
