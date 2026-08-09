// ---------------------------------------------------------------------------
// DIAD AGENT — WebSocket protocol types
// Mirrors agent/src/daemon/protocol.rs exactly (version v1, snake_case tags).
// ---------------------------------------------------------------------------

export const PROTOCOL_VERSION = 1;

// ---- Client → daemon -------------------------------------------------------

export type ClientMsg =
  | { type: "chat.send"; id: string; text: string }
  | { type: "tool.approve"; id: string }
  | { type: "tool.reject"; id: string }
  | { type: "edit.approve"; id: string }
  | { type: "edit.reject"; id: string }
  | { type: "spawn.tui" }
  | { type: "secret.submit"; id: string; value: string }
  | { type: "secret.cancel"; id: string }
  | { type: "file.read"; id: string; path: string }
  | { type: "file.write"; id: string; path: string; content: string }
  | { type: "file.list"; id: string; path: string }
  | { type: "term.input"; data: string }
  | { type: "term.resize"; cols: number; rows: number }
  | { type: "provider.switch"; name: string }
  | { type: "model.switch"; model: string }
  | { type: "provider.connect"; id: string; name: string; key: string }
  | { type: "agent.delegate"; id: string; agent: string; task: string }
  | { type: "config.set"; key: string; value: unknown }
  | { type: "providers.list"; id: string }
  | { type: "stats.get"; id: string }
  | { type: "models.list"; id: string; provider: string };

export interface Envelope<T> {
  v: number;
  msg: T;
}

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
    }
  | { type: "term.output"; data: string }
  | { type: "secret.request"; id: string; label: string }
  | { type: "confirm.request"; id: string; label: string }
  | { type: "stream.done"; id: string }
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
    }
  | { type: "file.content"; id: string; path: string; content: string }
  | { type: "files"; id: string; path: string; entries: FileEntryDto[] }
  | { type: "file.written"; id: string; path: string };

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
  | "projects";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
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
  loaded: boolean;
}

export type OnboardingStep =
  | { kind: "welcome" }
  | { kind: "provider" }
  | { kind: "wifi" }
  | { kind: "done" };
