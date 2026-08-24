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
  ClipboardEntryDto,
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
  TrashEntryDto,
  ViewId,
} from "../lib/types";
import { nextId } from "../lib/format";

function saveChatToStorage(messages: ChatMessage[]): void {
  try {
    const trimmed = messages.slice(-200).map((m) => {
      // Never persist base64 data URLs — a single screenshot can be megabytes
      // and blow the ~5 MB localStorage quota. The live in-memory message
      // keeps its images; only the persisted copy is stripped.
      if (m.images?.some((u) => u.startsWith("data:"))) {
        return { ...m, images: m.images.filter((u) => !u.startsWith("data:")) };
      }
      return m;
    });
    localStorage.setItem("diad.chat.messages", JSON.stringify(trimmed));
  } catch {
    // quota exceeded or private browsing
  }
}

function loadChatFromStorage(): ChatMessage[] {
  try {
    const raw = localStorage.getItem("diad.chat.messages");
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

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
  /** Monotonic counter of total PTY bytes appended (see appendTermOutput). */
  termSeq: number;
  termCols: number;
  termRows: number;

  // -- A7 parity: clipboard history + trash -------------------------------------
  clipboard: ClipboardEntryDto[];
  trash: TrashEntryDto[];
  trashLoading: boolean;

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
  dndEnabled: false,
  nightLightTemp: 0,
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
  clearMessages: () => void;
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

  // A7 parity: clipboard history + trash
  setClipboard: (items: ClipboardEntryDto[]) => void;
  setTrash: (items: TrashEntryDto[]) => void;
  setTrashLoading: (v: boolean) => void;

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
    dnd_enabled?: boolean;
    night_light_temp?: number;
  }) => void;
  setAgents: (a: string[]) => void;
}

export type Store = StoreState & StoreActions;

// Streaming token buffer: pending text per message id + its scheduled rAF.
const tokenBuffer = new Map<string, { text: string; raf: number | null }>();

function flushTokenBuffer(id: string): void {
  const buf = tokenBuffer.get(id);
  tokenBuffer.delete(id);
  if (!buf || !buf.text) return;
  useStore.setState((s) => ({
    messages: s.messages.map((m) =>
      m.id === id ? { ...m, content: m.content + buf.text, streaming: true } : m,
    ),
  }));
}

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

  messages: loadChatFromStorage(),
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
  termSeq: 0,
  termCols: 80,
  termRows: 24,

  clipboard: [],
  trash: [],
  trashLoading: false,

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

  // chat
  clearMessages: () => {
    saveChatToStorage([]);
    set({ messages: [] });
  },
  appendMessage: (m) => {
    const id = m.id ?? nextId("msg");
    set((s) => {
      const next = [
        ...s.messages,
        { ...m, id, tools: [], diffs: [], timestamp: Date.now() },
      ];
      saveChatToStorage(next);
      return { messages: next };
    });
    return id;
  },

  // Buffered token appends: tokens accumulate in a per-message buffer and
  // are flushed to the store at most once per animation frame, so a fast
  // token stream doesn't remap the whole messages array on every token.
  appendToken: (id, text) => {
    const buffers = tokenBuffer;
    const buf = buffers.get(id) ?? { text: "", raf: null };
    buf.text += text;
    if (buf.raf === null && typeof requestAnimationFrame === "function") {
      buf.raf = requestAnimationFrame(() => {
        buf.raf = null;
        flushTokenBuffer(id);
      });
    } else if (buf.raf === null) {
      // No rAF available (SSR/tests): apply synchronously.
      set((s) => ({
        messages: s.messages.map((m) =>
          m.id === id ? { ...m, content: m.content + buf.text } : m,
        ),
      }));
      buffers.delete(id);
      return;
    }
    buffers.set(id, buf);
  },

  finishStream: (id) => {
    // Flush any still-buffered tokens so the final content is complete
    // before the streaming flag is cleared.
    const pending = tokenBuffer.get(id);
    if (pending?.text) {
      if (pending.raf !== null && typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(pending.raf);
      }
      tokenBuffer.delete(id);
      set((s) => ({
        messages: s.messages.map((m) =>
          m.id === id ? { ...m, content: m.content + pending.text } : m,
        ),
      }));
    }
    return set((s) => {
      const next = s.messages.map((m) =>
        m.id === id ? { ...m, streaming: false } : m,
      );
      saveChatToStorage(next);
      return {
        streaming: s.streaming && s.messages.some((m) => m.id === id),
        messages: next,
      };
    });
  },

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
      // Monotonic byte counter: lets TerminalPanel compute the exact new
      // delta even after the ring buffer truncated old bytes (fixes the
      // "terminal freezes at 64 KB" bug where length stayed pinned).
      termSeq: s.termSeq + data.length,
    })),

  setTermSize: (cols, rows) => set({ termCols: cols, termRows: rows }),

  setClipboard: (items) => set({ clipboard: items }),
  setTrash: (items) => set({ trash: items, trashLoading: false }),
  setTrashLoading: (v) => set({ trashLoading: v }),

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
        ...(c.dnd_enabled !== undefined ? { dndEnabled: c.dnd_enabled } : {}),
        ...(c.night_light_temp !== undefined ? { nightLightTemp: c.night_light_temp } : {}),
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
