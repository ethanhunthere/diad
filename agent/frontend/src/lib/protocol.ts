// ---------------------------------------------------------------------------
// DIAD AGENT — WebSocket JSON protocol (v1)
// Mirror of agent/src/daemon/protocol.rs. Every client message is wrapped in
// an envelope { v: 1, type, ... }.
// ---------------------------------------------------------------------------

export const PROTOCOL_VERSION = 1;

// ---- Client → daemon ------------------------------------------------------

export type ClientMsg =
  | { type: "chat.send"; id: string; text: string }
  | { type: "tool.approve"; id: string }
  | { type: "tool.reject"; id: string }
  | { type: "edit.approve"; id: string }
  | { type: "edit.reject"; id: string }
  | { type: "spawn_tui" }
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
  | { type: "models.list"; id: string; provider: string }
  | { type: "agents.list"; id: string };

export interface Envelope {
  v: number;
  msg: ClientMsg;
}

// ---- Daemon → client ------------------------------------------------------

export interface ProviderEntry {
  name: string;
  needs_key: boolean;
  has_key: boolean;
}

export interface FileEntry {
  name: string;
  kind: "file" | "dir" | "symlink";
  size: number;
  git?: string;
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
  | { type: "providers"; id: string; entries: ProviderEntry[] }
  | { type: "models"; id: string; provider: string; models: string[]; current: string }
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
  | { type: "error"; id?: string | null; message: string }
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
  | { type: "files"; id: string; path: string; entries: FileEntry[] }
  | { type: "file.written"; id: string; path: string };

// ---- Derived UI models ----------------------------------------------------

export type ChatRole = "user" | "assistant" | "system" | "tool";

export interface ToolCallCard {
  callId: string;
  name: string;
  args: unknown;
  confirmRequired: boolean;
  pending: boolean;
  success?: boolean;
  output?: string;
}

export interface EditDiffCard {
  id: string;
  path: string;
  diff: string;
  resolved?: boolean;
  accepted?: boolean;
}

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  toolCalls: ToolCallCard[];
  diffs: EditDiffCard[];
  done: boolean;
  error?: boolean;
  ts: number;
}

export interface Toast {
  id: number;
  kind: "info" | "success" | "warn" | "error";
  text: string;
}

export type View = "chat" | "explorer" | "editor" | "terminal" | "agents" | "settings";

export type WsStatus = "connecting" | "connected" | "disconnected";
