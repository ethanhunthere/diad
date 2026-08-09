// ---------------------------------------------------------------------------
// Global zustand store — holds every UI model and dispatches ServerMsg.
// ---------------------------------------------------------------------------

import { create } from "zustand";

import {
  ChatMessage,
  EditDiffCard,
  FileEntry,
  ProviderEntry,
  ServerMsg,
  Toast,
  ToolCallCard,
  View,
  WsStatus,
} from "./lib/protocol";
import { daemon, newId } from "./lib/ws";

let toastSeq = 0;

interface Store {
  // ---- connection -----------------------------------------------------
  ws: WsStatus;
  provider: string;
  model: string;
  connected: boolean;
  streaming: boolean;
  daemonVersion: string;

  // ---- chat -----------------------------------------------------------
  messages: ChatMessage[];
  input: string;
  activeTurnId: string | null; // assistant message id currently streaming
  pendingSecret: { id: string; label: string } | null;
  pendingConfirm: { id: string; label: string } | null;

  // ---- providers / agents / stats -------------------------------------
  providers: ProviderEntry[];
  models: string[];
  agents: string[];
  stats: { tokens: number; latency: number; battery: string; wifi: string; cwd: string };

  // ---- explorer / editor ----------------------------------------------
  cwd: string;
  tree: FileEntry[];
  dirs: Record<string, FileEntry[]>;
  loadingDir: string | null;
  openTabs: { path: string; content: string; dirty: boolean }[];
  activeTab: string | null;

  // ---- ui -------------------------------------------------------------
  view: View;
  sidebarOpen: boolean;
  paletteOpen: boolean;
  providerPickerOpen: boolean;
  onboarding: boolean;
  toasts: Toast[];

  // ---- actions --------------------------------------------------------
  sendChat(text: string): void;
  approveTool(id: string): void;
  rejectTool(id: string): void;
  approveEdit(id: string): void;
  rejectEdit(id: string): void;
  submitSecret(value: string): void;
  cancelSecret(): void;
  answerConfirm(accept: boolean): void;
  switchProvider(name: string): void;
  switchModel(model: string): void;
  connectProvider(name: string, key: string): void;
  delegate(agent: string, task: string): void;
  setConfig(key: string, value: unknown): void;
  requestProviders(): void;
  requestModels(provider: string): void;
  requestAgents(): void;
  refreshStats(): void;
  readFile(path: string): void;
  writeFile(path: string, content: string): void;
  listDir(path: string): void;
  termInput(data: string): void;
  termResize(cols: number, rows: number): void;
  spawnTui(): void;

  setView(v: View): void;
  toggleSidebar(): void;
  setPaletteOpen(v: boolean): void;
  setProviderPickerOpen(v: boolean): void;
  setOnboarding(v: boolean): void;
  setInput(v: string): void;
  openTab(path: string): void;
  closeTab(path: string): void;
  setTabDirty(path: string, dirty: boolean): void;
  toast(kind: Toast["kind"], text: string): void;
  dismissToast(id: number): void;
  dispatch(msg: ServerMsg): void;
}

/** Default prompt shown when the assistant has nothing to say yet. */
const WELCOME: ChatMessage = {
  id: "welcome",
  role: "assistant",
  content:
    "Welcome to **DIAD AGENT**.\n\nAsk me anything — I can manage this system, edit files, run terminals, and delegate to local AI coding agents. Press `Ctrl+K` for the command palette.",
  toolCalls: [],
  diffs: [],
  done: true,
  ts: Date.now(),
};

export const useStore = create<Store>((set, get) => ({
  // ---- connection -----------------------------------------------------
  ws: "connecting",
  provider: "anthropic",
  model: "",
  connected: false,
  streaming: false,
  daemonVersion: "0.1.0",

  // ---- chat -----------------------------------------------------------
  messages: [WELCOME],
  input: "",
  activeTurnId: null,
  pendingSecret: null,
  pendingConfirm: null,

  // ---- providers / agents / stats -------------------------------------
  providers: [],
  models: [],
  agents: [],
  stats: { tokens: 0, latency: 0, battery: "n/a", wifi: "offline", cwd: "~" },

  // ---- explorer / editor ----------------------------------------------
  cwd: "~",
  tree: [],
  dirs: {},
  loadingDir: null,
  openTabs: [],
  activeTab: null,

  // ---- ui -------------------------------------------------------------
  view: "chat",
  sidebarOpen: true,
  paletteOpen: false,
  providerPickerOpen: false,
  onboarding: false,
  toasts: [],

  // ---- actions --------------------------------------------------------
  sendChat: (text) => {
    const t = text.trim();
    if (!t) return;
    const id = newId("turn");
    const userMsg: ChatMessage = {
      id: `user-${id}`,
      role: "user",
      content: t,
      toolCalls: [],
      diffs: [],
      done: true,
      ts: Date.now(),
    };
    const assistant: ChatMessage = {
      id,
      role: "assistant",
      content: "",
      toolCalls: [],
      diffs: [],
      done: false,
      ts: Date.now(),
    };
    set((s) => ({
      messages: [...s.messages, userMsg, assistant],
      activeTurnId: id,
      input: "",
    }));
    daemon.send({ type: "chat.send", id, text: t });
  },

  approveTool: (id) => daemon.send({ type: "tool.approve", id }),
  rejectTool: (id) => daemon.send({ type: "tool.reject", id }),

  approveEdit: (id) => {
    daemon.send({ type: "edit.approve", id });
    set((s) => ({
      messages: s.messages.map((m) => ({
        ...m,
        diffs: m.diffs.map((d) => (d.id === id ? { ...d, resolved: true, accepted: true } : d)),
      })),
    }));
  },
  rejectEdit: (id) => {
    daemon.send({ type: "edit.reject", id });
    set((s) => ({
      messages: s.messages.map((m) => ({
        ...m,
        diffs: m.diffs.map((d) => (d.id === id ? { ...d, resolved: true, accepted: false } : d)),
      })),
    }));
  },

  submitSecret: (value) => {
    const p = get().pendingSecret;
    if (!p) return;
    daemon.send({ type: "secret.submit", id: p.id, value });
    set({ pendingSecret: null });
  },
  cancelSecret: () => {
    const p = get().pendingSecret;
    if (!p) return;
    daemon.send({ type: "secret.cancel", id: p.id });
    set({ pendingSecret: null });
  },

  answerConfirm: (accept) => {
    const p = get().pendingConfirm;
    if (!p) return;
    daemon.send(accept ? { type: "tool.approve", id: p.id } : { type: "tool.reject", id: p.id });
    set({ pendingConfirm: null });
  },

  switchProvider: (name) => {
    daemon.send({ type: "provider.switch", name });
    set({ provider: name });
  },
  switchModel: (model) => {
    daemon.send({ type: "model.switch", model });
    set({ model });
  },
  connectProvider: (name, key) => {
    daemon.send({ type: "provider.connect", id: newId("conn"), name, key });
  },
  delegate: (agent, task) => {
    daemon.send({ type: "agent.delegate", id: newId("agent"), agent, task });
  },
  setConfig: (key, value) => daemon.send({ type: "config.set", key, value }),

  requestProviders: () => daemon.send({ type: "providers.list", id: newId("prov") }),
  requestModels: (provider) => daemon.send({ type: "models.list", id: newId("models"), provider }),
  requestAgents: () => daemon.send({ type: "agents.list", id: newId("agents") }),
  refreshStats: () => daemon.send({ type: "stats.get", id: newId("stats") }),

  readFile: (path) => daemon.send({ type: "file.read", id: newId("read"), path }),
  writeFile: (path, content) => daemon.send({ type: "file.write", id: newId("write"), path, content }),
  listDir: (path) => {
    set({ loadingDir: path });
    daemon.send({ type: "file.list", id: newId("list"), path });
  },
  termInput: (data) => daemon.send({ type: "term.input", data }),
  termResize: (cols, rows) => daemon.send({ type: "term.resize", cols, rows }),
  spawnTui: () => daemon.send({ type: "spawn_tui" }),

  // ---- ui -------------------------------------------------------------
  setView: (v) => set({ view: v }),
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  setPaletteOpen: (v) => set({ paletteOpen: v }),
  setProviderPickerOpen: (v) => set({ providerPickerOpen: v }),
  setOnboarding: (v) => set({ onboarding: v }),
  setInput: (v) => set({ input: v }),

  openTab: (path) => {
    const s = get();
    if (s.openTabs.some((t) => t.path === path)) {
      set({ activeTab: path });
      return;
    }
    const tab = { path, content: "", dirty: false };
    set((st) => ({ openTabs: [...st.openTabs, tab], activeTab: path }));
    s.readFile(path);
  },
  closeTab: (path) =>
    set((s) => {
      const idx = s.openTabs.findIndex((t) => t.path === path);
      const tabs = s.openTabs.filter((t) => t.path !== path);
      let active = s.activeTab;
      if (active === path) active = tabs[Math.max(0, idx - 1)]?.path ?? null;
      return { openTabs: tabs, activeTab: active };
    }),
  setTabDirty: (path, dirty) =>
    set((s) => ({
      openTabs: s.openTabs.map((t) => (t.path === path ? { ...t, dirty } : t)),
    })),

  toast: (kind, text) => {
    const id = ++toastSeq;
    set((s) => ({ toasts: [...s.toasts, { id, kind, text }] }));
    setTimeout(() => get().dismissToast(id), 4200);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  // ---- ServerMsg dispatch --------------------------------------------
  dispatch: (msg) => {
    switch (msg.type) {
      case "connected": {
        set({
          provider: msg.provider,
          model: msg.model,
          connected: msg.connected,
          streaming: msg.streaming,
          daemonVersion: msg.daemon_version,
        });
        get().requestProviders();
        get().requestAgents();
        get().refreshStats();
        break;
      }
      case "status":
        set({
          provider: msg.provider,
          model: msg.model,
          connected: msg.connected,
          streaming: msg.streaming,
          daemonVersion: msg.daemon_version,
        });
        break;
      case "token": {
        const turn = get().activeTurnId;
        if (!turn) break;
        set((s) => ({
          messages: s.messages.map((m) =>
            m.id === turn ? { ...m, content: m.content + msg.text } : m
          ),
        }));
        break;
      }
      case "stream.done": {
        const turn = get().activeTurnId;
        set((s) => ({
          messages: s.messages.map((m) =>
            m.id === turn ? { ...m, done: true } : m
          ),
          activeTurnId: null,
          streaming: false,
        }));
        break;
      }
      case "tool_use": {
        const card: ToolCallCard = {
          callId: msg.call_id,
          name: msg.name,
          args: msg.args,
          confirmRequired: msg.confirm_required,
          pending: true,
        };
        set((s) => ({
          messages: s.messages.map((m) =>
            m.id === s.activeTurnId ? { ...m, toolCalls: [...m.toolCalls, card] } : m
          ),
        }));
        break;
      }
      case "tool_result": {
        set((s) => ({
          messages: s.messages.map((m) => ({
            ...m,
            toolCalls: m.toolCalls.map((c) =>
              c.callId === msg.call_id
                ? { ...c, pending: false, success: msg.success, output: msg.output }
                : c
            ),
          })),
        }));
        break;
      }
      case "edit_diff": {
        const card: EditDiffCard = { id: msg.id, path: msg.path, diff: msg.diff };
        set((s) => ({
          messages: s.messages.map((m) =>
            m.id === s.activeTurnId ? { ...m, diffs: [...m.diffs, card] } : m
          ),
        }));
        break;
      }
      case "secret.request":
        set({ pendingSecret: { id: msg.id, label: msg.label } });
        break;
      case "confirm.request":
        set({ pendingConfirm: { id: msg.id, label: msg.label } });
        break;
      case "system": {
        const sys: ChatMessage = {
          id: newId("sys"),
          role: "system",
          content: msg.message,
          toolCalls: [],
          diffs: [],
          done: true,
          ts: Date.now(),
        };
        set((s) => ({ messages: [...s.messages, sys] }));
        break;
      }
      case "error": {
        const err: ChatMessage = {
          id: newId("err"),
          role: "system",
          content: `✗ ${msg.message}`,
          toolCalls: [],
          diffs: [],
          done: true,
          error: true,
          ts: Date.now(),
        };
        set((s) => ({
          messages: [...s.messages, err],
          activeTurnId: null,
          streaming: false,
        }));
        break;
      }
      case "providers":
        set({ providers: msg.entries });
        break;
      case "models":
        set({ models: msg.models });
        if (msg.current) set({ model: msg.current });
        break;
      case "agents":
        set({ agents: msg.agents });
        break;
      case "stats":
        set({
          stats: {
            tokens: msg.tokens,
            latency: msg.latency_ms,
            battery: msg.battery,
            wifi: msg.wifi,
            cwd: msg.cwd,
          },
        });
        break;
      case "file.content": {
        set((s) => ({
          openTabs: s.openTabs.map((t) =>
            t.path === msg.path ? { ...t, content: msg.content, dirty: false } : t
          ),
        }));
        break;
      }
      case "file.written": {
        get().toast("success", `Saved ${msg.path}`);
        break;
      }
      case "files": {
        set((s) => ({
          tree: s.loadingDir === msg.path ? msg.entries : s.tree,
          cwd: s.loadingDir === msg.path ? msg.path : s.cwd,
          dirs: { ...s.dirs, [msg.path]: msg.entries },
          loadingDir: null,
        }));
        break;
      }
      case "term.output": {
        // consumed by the TerminalPanel via its own subscription
        break;
      }
    }
  },
}));
