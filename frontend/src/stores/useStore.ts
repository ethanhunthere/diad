import { create } from "zustand";
import { cacheFile, dropCache, readCache } from "../lib/filecache";

// Debounced per-file IndexedDB writes (spec 6.6): one write per second of
// typing, not one per keystroke.
const cacheTimers = new Map<string, ReturnType<typeof setTimeout>>();
function queueCacheWrite(path: string, content: string): void {
  const prev = cacheTimers.get(path);
  if (prev) clearTimeout(prev);
  cacheTimers.set(
    path,
    setTimeout(() => {
      cacheTimers.delete(path);
      void cacheFile(path, content);
    }, 1000),
  );
}
import type {
  ChatMessage,
  ConfirmPrompt,
  EditDiffCard,
  FileEntryDto,
  OnboardingStep,
  OpenFile,
  ProviderEntryDto,
  SecretPrompt,
  SettingsState,
  Toast,
  ToolCallCard,
  ViewId,
} from "../lib/types";
import { nextId } from "../lib/format";

// ---------------------------------------------------------------------------
// DIAD AGENT — global UI store (zustand)
// ---------------------------------------------------------------------------

export interface Stats {
  tokens: number;
  latency_ms: number;
  battery: string;
  wifi: string;
  cwd: string;
  idle_stage: string;
  rss_mb: number;
  perf_tier: string;
}

interface StoreState {
  // -- connection -----------------------------------------------------------
  wsConnected: boolean;
  daemonVersion: string;
  /** Set by useWebSocket once the socket is live. */
  send: ((msg: unknown) => void) | null;

  // -- provider / model ------------------------------------------------------
  provider: string;
  model: string;
  connected: boolean;
  streaming: boolean;
  providers: ProviderEntryDto[];
  models: string[];

  // -- views -----------------------------------------------------------------
  view: ViewId;
  activityBarCollapsed: boolean;
  paletteOpen: boolean;
  settingsOpen: boolean;

  // -- chat ------------------------------------------------------------------
  messages: ChatMessage[];
  input: string;
  pendingSecret: SecretPrompt | null;
  pendingConfirm: ConfirmPrompt | null;
  /** provider name awaiting a key (connect flow). */
  pendingConnectProvider: string | null;

  // -- files / editor ---------------------------------------------------------
  explorerPath: string;
  explorerEntries: FileEntryDto[];
  explorerLoading: boolean;
  openFiles: OpenFile[];
  activeFile: string | null;

  // -- terminal ----------------------------------------------------------------
  termBuffer: string;
  termCols: number;
  termRows: number;

  // -- stats / status ----------------------------------------------------------
  stats: Stats;
  workspace: string;

  // -- misc ---------------------------------------------------------------------
  toasts: Toast[];
  onboarding: OnboardingStep | null;
  settings: SettingsState;
  agents: string[];
}

const initialSettings: SettingsState = {
  theme: "copper",
  defaultProvider: "",
  defaultModel: "",
  storagePath: "",
  firewall: true,
  // Persisted locally (spec 2.7/2.9): the daemon never needs these.
  fontScale: Math.min(2, Math.max(0.8, parseFloat(localStorage.getItem("diad.fontScale") ?? "1") || 1)),
  density: ((): SettingsState["density"] => {
    const d = localStorage.getItem("diad.density");
    return d === "compact" || d === "comfortable" ? d : "auto";
  })(),
  // THE LAW F-15: daemon-held prefs, overwritten by Status.config on connect.
  confirmDestructive: true,
  streamTokens: true,
  maxHistory: 100,
  autoSaveInterval: 0,
  loaded: false,
};

interface StoreActions {
  // connection
  setSend: (send: (msg: unknown) => void) => void;
  setWsConnected: (v: boolean) => void;

  // provider/model
  setProviderModel: (p: { provider?: string; model?: string; connected?: boolean; streaming?: boolean; version?: string }) => void;
  setProviders: (entries: ProviderEntryDto[]) => void;
  setModels: (models: string[], current: string) => void;

  // view
  setView: (v: ViewId) => void;
  toggleActivityBar: () => void;
  setPaletteOpen: (v: boolean) => void;

  // chat
  appendMessage: (m: Omit<ChatMessage, "tools" | "diffs" | "timestamp" | "id"> & { id?: string }) => string;
  appendToken: (id: string, text: string) => void;
  finishStream: (id: string) => void;
  setInput: (v: string) => void;
  addToolCard: (msgId: string, card: Omit<ToolCallCard, "status" | "collapsed">) => void;
  resolveToolCard: (requestId: string, status: ToolCallCard["status"], result?: { success: boolean; output: string; status_line: string }) => void;
  addDiffCard: (msgId: string, card: Omit<EditDiffCard, "status">) => void;
  resolveDiffCard: (requestId: string, status: EditDiffCard["status"]) => void;
  toggleToolCard: (msgId: string, requestId: string) => void;
  setPendingSecret: (p: SecretPrompt | null) => void;
  setPendingConfirm: (p: ConfirmPrompt | null) => void;
  setPendingConnectProvider: (p: string | null) => void;

  // files
  setExplorer: (path: string, entries: FileEntryDto[], loading: boolean) => void;
  openFile: (path: string, content: string) => void;
  setFileContent: (path: string, content: string) => void;
  closeFile: (path: string) => void;
  setActiveFile: (path: string | null) => void;
  markFileSaved: (path: string) => void;

  // terminal
  appendTermOutput: (data: string) => void;
  setTermSize: (cols: number, rows: number) => void;

  // stats
  setStats: (s: Partial<Stats>) => void;
  setWorkspace: (w: string) => void;

  // misc
  toast: (t: Omit<Toast, "id">) => void;
  dismissToast: (id: string) => void;
  setOnboarding: (s: OnboardingStep | null) => void;
  setSettings: (s: Partial<SettingsState>) => void;
  /** THE LAW F-15: merge a daemon config snapshot (Status.config) into settings. */
  mergeConfig: (c: {
    theme?: string;
    default_provider?: string;
    confirm_destructive?: boolean;
    stream_tokens?: boolean;
    max_history?: number;
    auto_save_interval?: number;
  }) => void;
  setAgents: (a: string[]) => void;
}

export type Store = StoreState & StoreActions;

export const useStore = create<Store>((set, get) => ({
  wsConnected: false,
  daemonVersion: "",
  send: null,

  provider: "",
  model: "",
  connected: false,
  streaming: false,
  providers: [],
  models: [],

  view: "chat",
  activityBarCollapsed: false,
  paletteOpen: false,
  settingsOpen: false,

  messages: [],
  input: "",
  pendingSecret: null,
  pendingConfirm: null,
  pendingConnectProvider: null,

  explorerPath: "",
  explorerEntries: [],
  explorerLoading: false,
  openFiles: [],
  activeFile: null,

  termBuffer: "",
  termCols: 80,
  termRows: 24,

  stats: { tokens: 0, latency_ms: 0, battery: "", wifi: "", cwd: "", idle_stage: "", rss_mb: 0, perf_tier: "" },
  workspace: "",

  toasts: [],
  onboarding: null,
  settings: initialSettings,
  agents: [],

  // ---------------------------------------------------------------------------

  setSend: (send) => set({ send }),

  setWsConnected: (v) => {
    set({ wsConnected: v });
    if (!v) {
      // Drop streaming flags so the UI never gets stuck mid-turn.
      set((s) => ({
        streaming: false,
        messages: s.messages.map((m) => (m.streaming ? { ...m, streaming: false } : m)),
      }));
    }
  },

  setProviderModel: (p) =>
    set((s) => ({
      provider: p.provider ?? s.provider,
      model: p.model ?? s.model,
      connected: p.connected ?? s.connected,
      streaming: p.streaming ?? s.streaming,
      daemonVersion: p.version ?? s.daemonVersion,
    })),

  setProviders: (entries) => set({ providers: entries }),

  setModels: (models, current) => set({ models, model: current || models[0] || "" }),

  setView: (v) => set({ view: v }),
  toggleActivityBar: () => set((s) => ({ activityBarCollapsed: !s.activityBarCollapsed })),
  setPaletteOpen: (v) => set({ paletteOpen: v }),

  appendMessage: (m) => {
    const id = m.id ?? nextId("msg");
    set((s) => ({
      messages: [
        ...s.messages,
        { ...m, id, tools: [], diffs: [], timestamp: Date.now() },
      ],
    }));
    return id;
  },

  appendToken: (id, text) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === id ? { ...m, content: m.content + text, streaming: true } : m,
      ),
    })),

  finishStream: (id) =>
    set((s) => ({
      streaming: s.streaming && s.messages.some((m) => m.id === id),
      messages: s.messages.map((m) =>
        m.id === id ? { ...m, streaming: false } : m,
      ),
    })),

  setInput: (v) => set({ input: v }),

  addToolCard: (msgId, card) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === msgId
          ? {
              ...m,
              tools: [
                ...m.tools,
                {
                  ...card,
                  status: card.confirmRequired ? "pending" : "done",
                  collapsed: false,
                },
              ],
            }
          : m,
      ),
    })),

  resolveToolCard: (requestId, status, result) =>
    set((s) => ({
      messages: s.messages.map((m) => ({
        ...m,
        tools: m.tools.map((t) =>
          t.requestId === requestId
            ? {
                ...t,
                status,
                resultOutput: result?.output,
                resultSuccess: result?.success,
                resultStatus: result?.status_line,
              }
            : t,
        ),
      })),
    })),

  addDiffCard: (msgId, card) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === msgId
          ? { ...m, diffs: [...m.diffs, { ...card, status: "pending" }] }
          : m,
      ),
    })),

  resolveDiffCard: (requestId, status) =>
    set((s) => ({
      messages: s.messages.map((m) => ({
        ...m,
        diffs: m.diffs.map((d) => (d.requestId === requestId ? { ...d, status } : d)),
      })),
    })),

  toggleToolCard: (msgId, requestId) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === msgId
          ? {
              ...m,
              tools: m.tools.map((t) =>
                t.requestId === requestId ? { ...t, collapsed: !t.collapsed } : t,
              ),
            }
          : m,
      ),
    })),

  setPendingSecret: (p) => set({ pendingSecret: p }),
  setPendingConfirm: (p) => set({ pendingConfirm: p }),
  setPendingConnectProvider: (p) => set({ pendingConnectProvider: p }),

  setExplorer: (path, entries, loading) =>
    set({ explorerPath: path, explorerEntries: entries, explorerLoading: loading }),

  openFile: (path, content) => {
    set((s) => {
      const exists = s.openFiles.some((f) => f.path === path);
      return {
        openFiles: exists
          ? s.openFiles
          : [...s.openFiles, { path, content, dirty: false }],
        activeFile: path,
        view: "editor",
      };
    });
    // Restore a locally cached unsaved edit if one exists (spec 6.6).
    void readCache(path).then((cached) => {
      if (cached === null) return;
      const current = get().openFiles.find((f) => f.path === path);
      if (current && !current.dirty && cached !== current.content) {
        set((s) => ({
          openFiles: s.openFiles.map((f) =>
            f.path === path ? { ...f, content: cached, dirty: true } : f,
          ),
        }));
      }
    });
  },

  setFileContent: (path, content) => {
    set((s) => ({
      openFiles: s.openFiles.map((f) => (f.path === path ? { ...f, content, dirty: true } : f)),
    }));
    queueCacheWrite(path, content);
  },

  closeFile: (path) =>
    set((s) => {
      const idx = s.openFiles.findIndex((f) => f.path === path);
      const openFiles = s.openFiles.filter((f) => f.path !== path);
      let activeFile = s.activeFile;
      if (activeFile === path) {
        const next = openFiles[Math.max(0, idx - 1)];
        activeFile = next ? next.path : null;
        if (!activeFile) {
          set({ view: s.view === "editor" ? "explorer" : s.view });
        }
      }
      return { openFiles, activeFile };
    }),

  setActiveFile: (path) => set({ activeFile: path }),

  markFileSaved: (path) => {
    set((s) => ({
      openFiles: s.openFiles.map((f) => (f.path === path ? { ...f, dirty: false } : f)),
    }));
    void dropCache(path);
  },

  appendTermOutput: (data) =>
    set((s) => ({
      termBuffer: (s.termBuffer + data).slice(-(64 * 1024)),
    })),

  setTermSize: (cols, rows) => set({ termCols: cols, termRows: rows }),

  setStats: (partial) => set((s) => ({ stats: { ...s.stats, ...partial } })),
  setWorkspace: (w) => set({ workspace: w }),

  toast: (t) => {
    const id = nextId("toast");
    set((s) => ({ toasts: [...s.toasts.slice(-4), { ...t, id }] }));
    if (t.ttl > 0) {
      setTimeout(() => get().dismissToast(id), t.ttl);
    }
  },

  dismissToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  setOnboarding: (s) => set({ onboarding: s }),
  setSettings: (s) => set((state) => ({ settings: { ...state.settings, ...s } })),
  // THE LAW F-15: Status.config arrives as snake_case daemon keys; map onto
  // the camelCase settings fields the panel binds to.
  mergeConfig: (c) =>
    set((state) => ({
      settings: {
        ...state.settings,
        ...(c.theme !== undefined ? { theme: c.theme } : {}),
        ...(c.default_provider !== undefined ? { defaultProvider: c.default_provider } : {}),
        ...(c.confirm_destructive !== undefined ? { confirmDestructive: c.confirm_destructive } : {}),
        ...(c.stream_tokens !== undefined ? { streamTokens: c.stream_tokens } : {}),
        ...(c.max_history !== undefined ? { maxHistory: c.max_history } : {}),
        ...(c.auto_save_interval !== undefined ? { autoSaveInterval: c.auto_save_interval } : {}),
        loaded: true,
      },
    })),
  setAgents: (a) => set({ agents: a }),
}));

/**
 * Convenience: send a FLAT protocol-v1 envelope (`{ v: 1, ...msg }`) if the
 * socket is live. The daemon's `Envelope` flattens the message into the top
 * level — never nest it under a `msg` key.
 */
export function sendMsg(msg: import("../lib/types").ClientMsg): boolean {
  const s = useStore.getState();
  if (!s.send) return false;
  s.send({ v: 1, ...msg });
  return true;
}
