import { useEffect, useState } from "react";
import { Save, RotateCcw, Folder, Shield, KeyRound } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId } from "../lib/format";

const THEMES = [
  { id: "monochrome", label: "Industrial Basalt", swatch: "#B32032" },
  { id: "slate", label: "Minimal Slate", swatch: "#94A3B8" },
  { id: "obsidian", label: "OLED Black", swatch: "#FFFFFF" },
  { id: "cyber", label: "Cyber Cobalt", swatch: "#38BDF8" },
  { id: "emerald", label: "Emerald Matrix", swatch: "#38A169" },
];

/**
 * Settings view — theme accent, default provider/model, keybindings
 * reference, storage path, and the offline firewall toggle. Changes
 * persist via config_set to the daemon.
 */
export function SettingsPanel() {
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  const wsConnected = useStore((s) => s.wsConnected);
  const providers = useStore((s) => s.providers);
  const toast = useStore((s) => s.toast);

  const [saved, setSaved] = useState<string | null>(null);
  // THE LAW F-15 compositor intents (local draft state; committed on Apply).
  const [waybarMode, setWaybarMode] = useState<"compact" | "full" | "">("");
  const [kbFrom, setKbFrom] = useState("");
  const [kbTo, setKbTo] = useState("");
  const stats = useStore((s) => s.stats);

  const persist = (key: string, value: unknown, label: string) => {
    sendMsg({ type: "config_set", key, value });
    setSaved(label);
    setTimeout(() => setSaved(null), 1500);
  };

  const changeAccent = (id: string) => {
    setSettings({ theme: id });
    document.documentElement.style.setProperty("--accent", THEMES.find((t) => t.id === id)?.swatch ?? "#B32032");
    persist("theme", id, "theme");
  };

  // Load stored accent on mount.
  useEffect(() => {
    const t = THEMES.find((x) => x.id === settings.theme)?.swatch ?? "#B32032";
    document.documentElement.style.setProperty("--accent", t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // GUI Personalization State
  const [fontFamily, setFontFamily] = useState<string>(() => localStorage.getItem("diad.font_family") || "Inter");
  const [borderRadius, setBorderRadius] = useState<string>(() => localStorage.getItem("diad.border_radius") || "clean");
  const [customAccent, setCustomAccent] = useState<string>(() => localStorage.getItem("diad.custom_accent") || "#E4E4E7");

  // TUI Personalization State (/tuicolor)
  const [tuiTheme, setTuiTheme] = useState<string>(() => localStorage.getItem("diad.tui_theme") || "monochrome");
  const [tuiBorder, setTuiBorder] = useState<"single" | "double" | "rounded" | "heavy" | "ascii">("rounded");
  const [tuiCursor, setTuiCursor] = useState<"block" | "underline" | "bar">("block");

  // Apply font family live
  useEffect(() => {
    document.body.style.fontFamily = fontFamily === "JetBrains Mono"
      ? '"JetBrains Mono", monospace'
      : fontFamily === "Fira Code"
      ? '"Fira Code", monospace'
      : fontFamily === "Geist"
      ? '"Geist", -apple-system, sans-serif'
      : fontFamily === "System Sans"
      ? 'system-ui, -apple-system, sans-serif'
      : '"Inter", system-ui, -apple-system, sans-serif';
    localStorage.setItem("diad.font_family", fontFamily);
  }, [fontFamily]);

  // Apply border radius live
  useEffect(() => {
    const root = document.documentElement;
    if (borderRadius === "sharp") {
      root.style.setProperty("--radius-panel", "0px");
      root.style.setProperty("--radius-control", "0px");
    } else if (borderRadius === "rounded") {
      root.style.setProperty("--radius-panel", "18px");
      root.style.setProperty("--radius-control", "12px");
    } else {
      root.style.setProperty("--radius-panel", "12px");
      root.style.setProperty("--radius-control", "8px");
    }
    localStorage.setItem("diad.border_radius", borderRadius);
  }, [borderRadius]);

  const applyTuiTheme = (tName: string) => {
    setTuiTheme(tName);
    localStorage.setItem("diad.tui_theme", tName);
    persist("tui_theme", tName, `TUI theme: ${tName}`);
    toast({ kind: "info", title: "TUI Personalization", message: `TUI theme set to ${tName}. Use /tuicolor in terminal.`, ttl: 2500 });
  };

  const spawnTui = () => {
    const send = useStore.getState().send;
    if (send) send({ v: 1, type: "spawn_tui" });
    else toast({ kind: "warn", title: "Offline", message: "Daemon not connected", ttl: 2000 });
  };

  // TUI Theme Color Map for the Live Preview
  const TUI_THEME_PREVIEWS: Record<string, { bg: string; fg: string; accent: string; bar: string; border: string }> = {
    monochrome: { bg: "#09090b", fg: "#f4f4f5", accent: "#e4e4e7", bar: "#18181b", border: "#27272a" },
    slate: { bg: "#0f172a", fg: "#e2e8f0", accent: "#94a3b8", bar: "#1e293b", border: "#334155" },
    tokyo_night: { bg: "#1a1b26", fg: "#c0caf5", accent: "#7dcfff", bar: "#24283b", border: "#414868" },
    catppuccin: { bg: "#1e1e2e", fg: "#cdd6f4", accent: "#cba6f7", bar: "#313244", border: "#45475a" },
    nord: { bg: "#2e3440", fg: "#eceff4", accent: "#88c0d0", bar: "#3b4252", border: "#4c566a" },
    gruvbox: { bg: "#282828", fg: "#ebdbb2", accent: "#fabd2f", bar: "#3c3836", border: "#504945" },
    matrix: { bg: "#050d05", fg: "#22c55e", accent: "#4ade80", bar: "#0f230f", border: "#166534" },
  };

  const activeTuiPreview = TUI_THEME_PREVIEWS[tuiTheme] || TUI_THEME_PREVIEWS.monochrome;

  // TUI Border Glyphs
  const BORDER_GLYPHS = {
    single: { tl: "┌", tr: "┐", bl: "└", br: "┘", h: "─", v: "│" },
    double: { tl: "╔", tr: "╗", bl: "╚", br: "╝", h: "═", v: "║" },
    rounded: { tl: "╭", tr: "╮", bl: "╰", br: "╯", h: "─", v: "│" },
    heavy: { tl: "┏", tr: "┓", bl: "┗", br: "┛", h: "━", v: "┃" },
    ascii: { tl: "+", tr: "+", bl: "+", br: "+", h: "-", v: "|" },
  }[tuiBorder];

  const CURSOR_GLYPH = {
    block: "█",
    underline: "_",
    bar: "│",
  }[tuiCursor];

  return (
    <div className="h-full overflow-y-auto px-5 py-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold text-ink tracking-tight">Personalization & Settings</h2>
          <p className="mt-0.5 text-[12px] text-dim">
            Customize GUI workspace aesthetics and TUI terminal theme (/tuicolor).
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="kbd text-[10px]">/tuicolor</span>
          <span className="kbd text-[10px]">/theme</span>
        </div>
      </div>

      {/* UNIFIED PERSONALIZATION STUDIO — SIDE BY SIDE (GUI LEFT, TUI RIGHT) */}
      <section className="mt-5">
        <div className="panel p-4">
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
            
            {/* ================= LEFT COLUMN: GUI PERSONALIZATION ================= */}
            <div className="space-y-4 pr-0 xl:pr-3 xl:border-r border-border-subtle/80">
              <div className="flex items-center justify-between pb-2 border-b border-border-subtle">
                <div className="flex items-center gap-2 text-[12px] font-semibold uppercase tracking-wider text-ink">
                  <span className="flex h-5 w-5 items-center justify-center rounded bg-surface-2 text-ink">🔲</span>
                  GUI Personalization (Workspace 1)
                </div>
                <span className="text-[10px] text-faint">2026 Modern Clean</span>
              </div>

              {/* GUI Theme Palettes (2026 Clean Aesthetics: Titanium, Monochrome, Slate, OLED Black) */}
              <div>
                <div className="text-[11.5px] font-medium text-dim">Color Theme Preset</div>
                <div className="mt-2 grid grid-cols-2 sm:grid-cols-3 gap-2">
                  {[
                    { id: "monochrome", label: "Titanium Mono", swatch: "#E4E4E7", desc: "Clean neutral 2026" },
                    { id: "slate", label: "Minimal Slate", swatch: "#94A3B8", desc: "Deep graphite & silver" },
                    { id: "obsidian", label: "OLED Black", swatch: "#FFFFFF", desc: "True black & crisp ink" },
                    { id: "light", label: "Studio Light", swatch: "#F4F4F5", desc: "Clean architectural" },
                    { id: "cyber", label: "Cyber Platinum", swatch: "#38BDF8", desc: "Slate & ice cyan" },
                    { id: "copper", label: "Heritage Bronze", swatch: "#C87F4A", desc: "Warm obsidian bronze" },
                  ].map((t) => (
                    <button
                      key={t.id}
                      className={`flex flex-col items-start p-2 rounded-lg border text-left transition-all ${
                        settings.theme === t.id
                          ? "border-ink bg-surface-3 shadow-sm ring-1 ring-white/10"
                          : "border-border-subtle bg-surface-2/60 hover:border-border-strong hover:bg-surface-2"
                      }`}
                      onClick={() => {
                        changeAccent(t.id);
                        setCustomAccent(t.swatch);
                        localStorage.setItem("diad.custom_accent", t.swatch);
                      }}
                    >
                      <div className="flex items-center gap-1.5 w-full">
                        <span className="h-2.5 w-2.5 rounded-full border border-white/20" style={{ background: t.swatch }} />
                        <span className="text-[11.5px] font-medium text-ink truncate">{t.label}</span>
                      </div>
                      <span className="text-[9.5px] text-faint mt-1 leading-tight">{t.desc}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* Accent Color Swatches */}
              <div>
                <div className="text-[11.5px] font-medium text-dim">Accent Highlight</div>
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  {[
                    { color: "#FFFFFF", name: "Pure White" },
                    { color: "#E4E4E7", name: "Titanium" },
                    { color: "#94A3B8", name: "Slate Silver" },
                    { color: "#38BDF8", name: "Ice Cyan" },
                    { color: "#34D399", name: "Emerald" },
                    { color: "#FBBF24", name: "Warm Amber" },
                    { color: "#A78BFA", name: "Violet" },
                  ].map((s) => (
                    <button
                      key={s.color}
                      className={`h-7 w-7 rounded-full border flex items-center justify-center transition-transform hover:scale-110 ${
                        customAccent === s.color ? "border-white ring-2 ring-white/30 scale-105" : "border-border-subtle"
                      }`}
                      style={{ background: s.color }}
                      title={s.name}
                      onClick={() => {
                        setCustomAccent(s.color);
                        localStorage.setItem("diad.custom_accent", s.color);
                        document.documentElement.style.setProperty("--accent", s.color);
                        toast({ kind: "info", title: "Accent Selected", message: s.name, ttl: 1500 });
                      }}
                    />
                  ))}
                  <div className="flex items-center gap-1.5 ml-2">
                    <input
                      type="color"
                      value={customAccent}
                      onChange={(e) => {
                        setCustomAccent(e.target.value);
                        localStorage.setItem("diad.custom_accent", e.target.value);
                        document.documentElement.style.setProperty("--accent", e.target.value);
                      }}
                      className="h-7 w-7 rounded-md cursor-pointer border border-border-subtle bg-transparent p-0"
                      title="Custom Color Picker"
                    />
                    <span className="font-mono text-[10px] text-faint">{customAccent}</span>
                  </div>
                </div>
              </div>

              {/* Typography Engine */}
              <div className="pt-2 border-t border-border-subtle/60">
                <div className="text-[11.5px] font-medium text-dim">Typography & Font Family</div>
                <div className="mt-2 grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                  {[
                    { id: "Inter", label: "Inter", style: "font-sans" },
                    { id: "JetBrains Mono", label: "JetBrains Mono", style: "font-mono" },
                    { id: "Geist", label: "Geist Clean", style: "font-sans" },
                    { id: "Fira Code", label: "Fira Code", style: "font-mono" },
                    { id: "System Sans", label: "System UI", style: "font-sans" },
                  ].map((f) => (
                    <button
                      key={f.id}
                      className={`p-2 rounded-md border text-left text-[11px] transition-all ${
                        fontFamily === f.id
                          ? "border-ink bg-surface-3 text-ink font-semibold"
                          : "border-border-subtle bg-surface-2/50 text-dim hover:border-border-strong hover:text-ink"
                      } ${f.style}`}
                      onClick={() => setFontFamily(f.id)}
                    >
                      {f.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Font Scale Slider */}
              <div className="pt-2 border-t border-border-subtle/60">
                <div className="flex items-center justify-between text-[11.5px]">
                  <span className="text-dim">Font Scaling</span>
                  <span className="font-mono text-ink font-medium">{settings.fontScale.toFixed(2)}x</span>
                </div>
                <input
                  type="range"
                  min={0.8}
                  max={1.6}
                  step={0.05}
                  value={settings.fontScale}
                  onChange={(e) => setSettings({ fontScale: parseFloat(e.target.value) })}
                  className="mt-1.5 w-full accent-white"
                />
              </div>

              {/* Corner Radius & UI Geometry */}
              <div className="pt-2 border-t border-border-subtle/60 flex items-center justify-between">
                <div>
                  <div className="text-[11.5px] text-dim">Window & Card Geometry</div>
                  <div className="text-[10px] text-faint">Corner radius rounding</div>
                </div>
                <div className="flex gap-1">
                  {[
                    { id: "sharp", label: "Sharp 0px" },
                    { id: "clean", label: "Clean 8px" },
                    { id: "rounded", label: "Smooth 14px" },
                  ].map((r) => (
                    <button
                      key={r.id}
                      className={`px-2 py-1 rounded text-[10.5px] border transition-all ${
                        borderRadius === r.id
                          ? "border-ink bg-surface-3 text-ink font-medium"
                          : "border-border-subtle bg-surface-2/40 text-dim hover:border-border-strong"
                      }`}
                      onClick={() => setBorderRadius(r.id)}
                    >
                      {r.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            {/* ================= RIGHT COLUMN: TUI PERSONALIZATION (/tuicolor) ================= */}
            <div className="space-y-4 pl-0 xl:pl-3">
              <div className="flex items-center justify-between pb-2 border-b border-border-subtle">
                <div className="flex items-center gap-2 text-[12px] font-semibold uppercase tracking-wider text-ink">
                  <span className="flex h-5 w-5 items-center justify-center rounded bg-surface-2 text-ink">📟</span>
                  TUI Personalization (/tuicolor)
                </div>
                <span className="text-[10px] text-faint">Terminal UI Studio</span>
              </div>

              {/* LIVE TUI TERMINAL PREVIEW BOX */}
              <div>
                <div className="text-[11.5px] font-medium text-dim mb-1.5 flex items-center justify-between">
                  <span>Interactive Terminal Preview</span>
                  <span className="text-[10px] text-faint font-mono">mode: Ratatui 80x24</span>
                </div>

                <div
                  className="rounded-lg p-3 font-mono text-[11px] leading-relaxed shadow-xl border select-none transition-colors"
                  style={{
                    backgroundColor: activeTuiPreview.bg,
                    borderColor: activeTuiPreview.border,
                    color: activeTuiPreview.fg,
                  }}
                >
                  {/* Top Bar Preview */}
                  <div
                    className="flex items-center justify-between px-2 py-0.5 rounded text-[10px] font-bold"
                    style={{ backgroundColor: activeTuiPreview.bar, color: activeTuiPreview.fg }}
                  >
                    <span>DIAD AGENT │ CLAUDE 3.7 SONNET │ CHAT</span>
                    <span style={{ color: activeTuiPreview.accent }}>● ONLINE</span>
                  </div>

                  {/* Terminal Border Simulation */}
                  <div className="my-2 text-[10px] leading-tight opacity-70" style={{ color: activeTuiPreview.accent }}>
                    {BORDER_GLYPHS.tl}{BORDER_GLYPHS.h.repeat(38)}{BORDER_GLYPHS.tr}
                  </div>

                  {/* Chat Content Simulation */}
                  <div className="space-y-1 my-1.5 text-[10.5px]">
                    <div className="opacity-80">[user] /tuicolor {tuiTheme}</div>
                    <div style={{ color: activeTuiPreview.accent }}>
                      [diad] TUI theme switched to {tuiTheme.toUpperCase()}. High-contrast clean rendering active.
                    </div>
                  </div>

                  <div className="my-2 text-[10px] leading-tight opacity-70" style={{ color: activeTuiPreview.accent }}>
                    {BORDER_GLYPHS.bl}{BORDER_GLYPHS.h.repeat(38)}{BORDER_GLYPHS.br}
                  </div>

                  {/* Bottom Input Line Simulation */}
                  <div
                    className="flex items-center gap-1 border-t pt-1.5 text-[10.5px]"
                    style={{ borderColor: activeTuiPreview.border }}
                  >
                    <span style={{ color: activeTuiPreview.accent }}>&gt;</span>
                    <span>Ready for command</span>
                    <span className="animate-pulse font-bold" style={{ color: activeTuiPreview.accent }}>
                      {CURSOR_GLYPH}
                    </span>
                  </div>
                </div>
              </div>

              {/* TUI Theme Presets */}
              <div>
                <div className="text-[11.5px] font-medium text-dim">TUI Color Palette (/tuicolor)</div>
                <div className="mt-2 grid grid-cols-2 sm:grid-cols-3 gap-2">
                  {[
                    { id: "monochrome", label: "Monochrome", fg: "#F4F4F5", bg: "#09090B", desc: "Pure B&W high contrast" },
                    { id: "slate", label: "Slate Minimal", fg: "#E2E8F0", bg: "#0F172A", desc: "Cool graphite terminal" },
                    { id: "tokyo_night", label: "Tokyo Night", fg: "#C0CAF5", bg: "#1A1B26", desc: "Slate & soft cyan" },
                    { id: "catppuccin", label: "Catppuccin", fg: "#CDD6F4", bg: "#1E1E2E", desc: "Smooth dark mocha" },
                    { id: "nord", label: "Nord Frost", fg: "#ECEFF4", bg: "#2E3440", desc: "Arctic glacial slate" },
                    { id: "gruvbox", label: "Gruvbox", fg: "#EBDBB2", bg: "#282828", desc: "Warm charcoal retro" },
                    { id: "matrix", label: "Matrix Green", fg: "#22C55E", bg: "#050D05", desc: "Phosphor terminal" },
                  ].map((t) => (
                    <button
                      key={t.id}
                      className={`flex flex-col items-start p-2 rounded-lg border text-left transition-all ${
                        tuiTheme === t.id
                          ? "border-ink bg-surface-3 ring-1 ring-white/10"
                          : "border-border-subtle bg-surface-2/60 hover:border-border-strong hover:bg-surface-2"
                      }`}
                      onClick={() => applyTuiTheme(t.id)}
                    >
                      <div className="flex items-center gap-1.5 w-full">
                        <span className="h-2.5 w-2.5 rounded-full border border-white/20" style={{ background: t.fg }} />
                        <span className="text-[11px] font-medium text-ink truncate">{t.label}</span>
                      </div>
                      <span className="text-[9.5px] text-faint mt-1 leading-tight">{t.desc}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* TUI Border Glyph Picker */}
              <div className="pt-2 border-t border-border-subtle/60 flex items-center justify-between">
                <div>
                  <div className="text-[11.5px] text-dim">TUI Box Borders</div>
                  <div className="text-[10px] text-faint">ASCII framing glyphs</div>
                </div>
                <div className="flex gap-1">
                  {(["rounded", "single", "double", "heavy", "ascii"] as const).map((b) => (
                    <button
                      key={b}
                      className={`px-2 py-1 rounded text-[10px] font-mono border transition-all ${
                        tuiBorder === b
                          ? "border-ink bg-surface-3 text-ink font-semibold"
                          : "border-border-subtle bg-surface-2/40 text-dim hover:border-border-strong"
                      }`}
                      onClick={() => setTuiBorder(b)}
                    >
                      {b === "rounded" ? "╭─╮" : b === "double" ? "╔═╗" : b === "heavy" ? "┏━┓" : b === "ascii" ? "+-+" : "┌─┐"}
                    </button>
                  ))}
                </div>
              </div>

              {/* TUI Cursor Shape */}
              <div className="pt-2 border-t border-border-subtle/60 flex items-center justify-between">
                <div>
                  <div className="text-[11.5px] text-dim">TUI Terminal Cursor</div>
                  <div className="text-[10px] text-faint">Prompt focus glyph</div>
                </div>
                <div className="flex gap-1.5">
                  {[
                    { id: "block", label: "Block █" },
                    { id: "underline", label: "Underline _" },
                    { id: "bar", label: "Bar │" },
                  ].map((c) => (
                    <button
                      key={c.id}
                      className={`px-2.5 py-1 rounded text-[10.5px] font-mono border transition-all ${
                        tuiCursor === c.id
                          ? "border-ink bg-surface-3 text-ink font-semibold"
                          : "border-border-subtle bg-surface-2/40 text-dim hover:border-border-strong"
                      }`}
                      onClick={() => setTuiCursor(c.id as any)}
                    >
                      {c.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Launch TUI Button */}
              <div className="pt-2 border-t border-border-subtle/60 flex items-center justify-between">
                <span className="text-[11px] text-dim">Open terminal client:</span>
                <button
                  className="btn py-1.5 px-3 text-[11px] font-medium flex items-center gap-1.5 border-border-strong"
                  onClick={spawnTui}
                >
                  <span>⚡ Launch Live TUI (foot)</span>
                </button>
              </div>
            </div>

          </div>
        </div>
      </section>

      {/* All AI Providers & API Keys Manager */}
      <section className="mt-5">
        <div className="mb-2 flex items-center justify-between text-[11px] uppercase tracking-wider text-faint">
          <span className="flex items-center gap-2"><KeyRound size={12} /> AI Providers & API Keys</span>
          <span className="text-[10px] text-dim">{providers.length} available</span>
        </div>
        <div className="panel p-3 space-y-3">
          <div className="text-[11.5px] text-dim">
            Configure your API keys. Keys are securely stored in the system vault and never exposed in plain text.
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-2.5">
            {[
              { id: "anthropic", label: "Anthropic", models: "Claude 3.7 Sonnet, 3.5 Sonnet, 3.5 Haiku, Opus", placeholder: "sk-ant-api..." },
              { id: "openai", label: "OpenAI", models: "GPT-4.5, GPT-4o, GPT-4o-mini, o1, o3-mini", placeholder: "sk-proj-..." },
              { id: "gemini", label: "Google Gemini", models: "Gemini 2.5 Pro, 2.5 Flash, 2.0 Flash, 1.5 Pro", placeholder: "AIzaSy..." },
              { id: "qwen", label: "Qwen / Alibaba", models: "Qwen 2.5 Max, Plus, Coder 32B, 72B", placeholder: "sk-..." },
              { id: "kimi", label: "Moonshot / Kimi", models: "Kimi K2, Moonshot v1 8k/32k/128k", placeholder: "sk-..." },
              { id: "deepseek", label: "DeepSeek", models: "DeepSeek V3, DeepSeek R1", placeholder: "sk-..." },
              { id: "groq", label: "Groq (Fast)", models: "Llama 3.3 70B, DeepSeek R1 Llama, Mixtral", placeholder: "gsk_..." },
              { id: "mistral", label: "Mistral AI", models: "Mistral Large 2, Small, Codestral, Pixtral", placeholder: "..." },
              { id: "openrouter", label: "OpenRouter", models: "Unified multi-model router (Claude, GPT, R1)", placeholder: "sk-or-v1-..." },
              { id: "ollama", label: "Ollama / Local LLM", models: "Qwen 2.5, DeepSeek R1, Llama 3.3 (Local)", placeholder: "http://localhost:11434" },
              { id: "together", label: "Together AI", models: "Open weights & fine-tunes", placeholder: "..." },
              { id: "xai", label: "xAI (Grok)", models: "Grok-2, Grok-Beta", placeholder: "xai-..." },
              { id: "perplexity", label: "Perplexity", models: "Sonar Pro, Sonar Reasoning", placeholder: "pplx-..." },
              { id: "cohere", label: "Cohere", models: "Command R+, Command Light", placeholder: "..." },
            ].map((p) => {
              const liveProv = providers.find((lp) => lp.name.toLowerCase() === p.id);
              const isConfigured = !!liveProv?.has_key;
              return (
                <div key={p.id} className="rounded-lg border border-border-subtle bg-surface-2/60 p-2.5 flex flex-col justify-between gap-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-1.5 font-medium text-[12.5px] text-ink">
                      <span className={`h-2 w-2 rounded-full ${isConfigured ? "bg-success shadow-[0_0_4px_rgba(111,191,139,0.8)]" : "bg-faint"}`} />
                      {p.label}
                    </div>
                    <span className="text-[10px] text-faint font-mono">
                      {isConfigured ? "✓ Configured" : "No key"}
                    </span>
                  </div>
                  <div className="text-[10.5px] text-dim">{p.models}</div>
                  <div className="flex items-center gap-1.5 mt-1">
                    <input
                      type="password"
                      className="control flex-1 text-[11px] py-1 px-2"
                      placeholder={p.placeholder}
                      id={`key-${p.id}`}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          const input = e.currentTarget;
                          const val = input.value.trim();
                          if (val) {
                            sendMsg({ type: "provider_connect", id: nextId("key"), name: p.id, key: val });
                            input.value = "";
                            toast({ kind: "info", title: p.label, message: "API key updated", ttl: 2500 });
                          }
                        }
                      }}
                    />
                    <button
                      className="btn py-1 px-2 text-[11px]"
                      onClick={() => {
                        const input = document.getElementById(`key-${p.id}`) as HTMLInputElement | null;
                        if (input && input.value.trim()) {
                          sendMsg({ type: "provider_connect", id: nextId("key"), name: p.id, key: input.value.trim() });
                          input.value = "";
                          toast({ kind: "info", title: p.label, message: "API key updated", ttl: 2500 });
                        }
                      }}
                    >
                      Save
                    </button>
                  </div>
                </div>
              );
            })}
          </div>

          <div className="mt-4 border-t border-border-subtle/80 pt-3 flex items-center justify-between gap-3">
            <div>
              <div className="text-[12px] text-ink">Default provider</div>
              <div className="text-[10.5px] text-faint">Primary provider for chat and tool routing.</div>
            </div>
            <select
              className="control w-44 text-[12px]"
              value={settings.defaultProvider}
              disabled={!wsConnected}
              onChange={(e) => {
                const v = e.target.value;
                setSettings({ defaultProvider: v });
                persist("default_provider", v, "default provider");
              }}
            >
              <option value="">— auto / default —</option>
              {providers.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </section>

      {/* Chat History & Data Retention */}
      <section className="mt-4">
        <div className="mb-2 flex items-center justify-between text-[11px] uppercase tracking-wider text-faint">
          <span>💬 Chat History & Memory</span>
          <span className="text-[10px] text-dim">{useStore.getState().messages.length} messages</span>
        </div>
        <div className="panel p-3 space-y-3">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-[12px] text-ink">Clear local chat history</div>
              <div className="text-[10.5px] text-faint">
                Wipes stored messages while preserving settings and vault keys.
              </div>
            </div>
            <button
              className="btn border-danger/40 text-danger hover:bg-danger/10 text-[11px] py-1 px-2.5"
              onClick={() => {
                useStore.getState().clearMessages();
                toast({ kind: "info", title: "Chat Cleared", message: "Local conversation history reset.", ttl: 2500 });
              }}
            >
              Clear Chat
            </button>
          </div>
        </div>
      </section>

      {/* Keybindings */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          ⌨ Keybindings
        </div>
        <div className="panel p-3">
          <table className="w-full text-left text-[12px]">
            <tbody>
              {[
                ["Ctrl+K", "Command palette"],
                ["Ctrl+B", "Toggle activity bar"],
                ["Ctrl+`", "Terminal"],
                ["Ctrl+S", "Save file"],
                ["F6", "Provider picker"],
                ["Ctrl+Shift+X", "Open TUI (foot)"],
                ["F9 (TUI)", "Open GUI"],
              ].map(([k, v]) => (
                <tr key={k} className="border-b border-border-subtle/60 last:border-0">
                  <td className="w-40 px-2 py-1.5">
                    <span className="kbd">{k}</span>
                  </td>
                  <td className="px-2 py-1.5 text-dim">{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* Storage */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <Folder size={12} /> Storage
        </div>
        <div className="panel p-3">
          <div className="flex items-center gap-2">
            <input
              className="control flex-1 font-mono text-[12px]"
              value={settings.storagePath}
              placeholder="~/.diad (default)"
              onChange={(e) => setSettings({ storagePath: e.target.value })}
            />
            <button
              className="btn"
              onClick={() => {
                persist("storage_path", settings.storagePath, "storage path");
              }}
            >
              <Save size={13} /> Apply
            </button>
          </div>
          <div className="mt-1.5 text-[10.5px] text-faint">
            Vault keys, config and history live here.
          </div>
        </div>
      </section>

      {/* Firewall */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <Shield size={12} /> Offline firewall
        </div>
        <div className="panel flex items-center justify-between p-3">
          <div>
            <div className="text-[12px] text-ink">Block network calls when offline</div>
            <div className="text-[10.5px] text-faint">
              Routes intents to local tools instead of the LLM when Wi-Fi drops.
            </div>
          </div>
          <button
            className={`relative h-6 w-11 rounded-full border transition-colors duration-150 ease-out ${
              settings.firewall
                ? "border-copper/60 bg-copper/25"
                : "border-border-strong bg-surface-2"
            }`}
            onClick={() => {
              const v = !settings.firewall;
              setSettings({ firewall: v });
              persist("firewall", v, "firewall");
            }}
            title={settings.firewall ? "Firewall on" : "Firewall off"}
          >
            <span
              className={`absolute top-0.5 h-5 w-5 rounded-full transition-all duration-150 ease-out ${
                settings.firewall
                  ? "left-[22px] bg-copper-hover"
                  : "left-0.5 bg-faint"
              }`}
            />
          </button>
        </div>
      </section>

      {/* THE LAW F-15: daemon-held behavior preferences. Values are reflected
          from the daemon's Status.config on connect, and each control persists
          immediately via config.set. */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          ⚙ Agent behavior
        </div>
        <div className="panel p-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-[12px] text-ink">Stream tokens</div>
              <div className="text-[10.5px] text-faint">
                Show LLM output token-by-token instead of a full response.
              </div>
            </div>
            <button
              className={`relative h-6 w-11 shrink-0 rounded-full border transition-colors duration-150 ease-out ${
                settings.streamTokens
                  ? "border-copper/60 bg-copper/25"
                  : "border-border-strong bg-surface-2"
              }`}
              onClick={() => {
                const v = !settings.streamTokens;
                setSettings({ streamTokens: v });
                persist("stream_tokens", v, "stream tokens");
              }}
              title={settings.streamTokens ? "Streaming on" : "Streaming off"}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full transition-all duration-150 ease-out ${
                  settings.streamTokens ? "left-[22px] bg-copper-hover" : "left-0.5 bg-faint"
                }`}
              />
            </button>
          </div>

          <div className="mt-3 flex items-center justify-between gap-3 border-t border-border-subtle/60 pt-3">
            <div>
              <div className="text-[12px] text-ink">Confirm destructive actions</div>
              <div className="text-[10.5px] text-faint">
                Ask before the agent deletes files, formats drives, or runs sudo.
              </div>
            </div>
            <button
              className={`relative h-6 w-11 shrink-0 rounded-full border transition-colors duration-150 ease-out ${
                settings.confirmDestructive
                  ? "border-copper/60 bg-copper/25"
                  : "border-border-strong bg-surface-2"
              }`}
              onClick={() => {
                const v = !settings.confirmDestructive;
                setSettings({ confirmDestructive: v });
                persist("confirm_destructive", v, "confirm destructive");
              }}
              title={settings.confirmDestructive ? "Confirmations on" : "Confirmations off"}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full transition-all duration-150 ease-out ${
                  settings.confirmDestructive ? "left-[22px] bg-copper-hover" : "left-0.5 bg-faint"
                }`}
              />
            </button>
          </div>

          <div className="mt-3 grid grid-cols-2 gap-3 border-t border-border-subtle/60 pt-3">
            <div>
              <div className="text-[11.5px] text-dim">Max history (messages)</div>
              <input
                type="number"
                min={1}
                max={10000}
                className="control mt-1 w-full"
                value={settings.maxHistory}
                onChange={(e) => {
                  const v = Math.max(1, Math.min(10000, parseInt(e.target.value || "0", 10) || 0));
                  setSettings({ maxHistory: v });
                }}
                onBlur={() => persist("max_history", settings.maxHistory, "max history")}
              />
              <div className="mt-1 text-[10px] text-faint">Applies on the next turn.</div>
            </div>
            <div>
              <div className="text-[11.5px] text-dim">Auto-save interval (s)</div>
              <input
                type="number"
                min={0}
                max={3600}
                className="control mt-1 w-full"
                value={settings.autoSaveInterval}
                onChange={(e) => {
                  const v = Math.max(0, Math.min(3600, parseInt(e.target.value || "0", 10) || 0));
                  setSettings({ autoSaveInterval: v });
                }}
                onBlur={() => persist("auto_save_interval", settings.autoSaveInterval, "auto-save interval")}
              />
              <div className="mt-1 text-[10px] text-faint">0 disables auto-save.</div>
            </div>
          </div>
        </div>
      </section>

      {/* A7 parity: mako do-not-disturb + gammastep night light. Persist
          through the same config.set surface as the other daemon prefs. */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          🌙 Desktop
        </div>
        <div className="panel p-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-[12px] text-ink">Do not disturb</div>
              <div className="text-[10.5px] text-faint">
                Quiet mako notifications (makoctl mode dnd).
              </div>
            </div>
            <button
              className={`relative h-6 w-11 shrink-0 rounded-full border transition-colors duration-150 ease-out ${
                settings.dndEnabled
                  ? "border-copper/60 bg-copper/25"
                  : "border-border-strong bg-surface-2"
              }`}
              onClick={() => {
                const v = !settings.dndEnabled;
                setSettings({ dndEnabled: v });
                persist("dnd_enabled", v, "do not disturb");
              }}
              title={settings.dndEnabled ? "Do not disturb on" : "Do not disturb off"}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full transition-all duration-150 ease-out ${
                  settings.dndEnabled ? "left-[22px] bg-copper-hover" : "left-0.5 bg-faint"
                }`}
              />
            </button>
          </div>

          <div className="mt-3 border-t border-border-subtle/60 pt-3">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-[12px] text-ink">Night light</div>
                <div className="text-[10.5px] text-faint">
                  gammastep colour temperature; 0 (left) is off.
                </div>
              </div>
              <span className="shrink-0 font-mono text-[11px] text-dim">
                {settings.nightLightTemp > 0 ? `${settings.nightLightTemp} K` : "off"}
              </span>
            </div>
            <input
              type="range"
              min={0}
              max={6500}
              step={100}
              className="mt-2 w-full accent-copper-hover"
              value={settings.nightLightTemp}
              onChange={(e) => {
                const v = parseInt(e.target.value, 10) || 0;
                setSettings({ nightLightTemp: v });
              }}
              onMouseUp={() =>
                persist("night_light", { enabled: settings.nightLightTemp > 0, temperature: settings.nightLightTemp }, "night light")
              }
              onTouchEnd={() =>
                persist("night_light", { enabled: settings.nightLightTemp > 0, temperature: settings.nightLightTemp }, "night light")
              }
              title="Night light temperature (0 = off)"
            />
            <div className="mt-1 flex justify-between text-[9.5px] text-faint">
              <span>0 — off</span>
              <span>3500 — warm</span>
              <span>6500 — daylight</span>
            </div>
          </div>
        </div>
      </section>

      {/* THE LAW F-15: compositor intent surfaces (chat intents exist too —
          "remap my keys", "set perf tier to high", "compact waybar"). */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          ⌨ Compositor
        </div>
        <div className="panel p-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-[12px] text-ink">Performance tier</div>
              <div className="text-[10.5px] text-faint">
                /etc/diad/perf-tier override (ULTRA_LOW → ULTRA).
              </div>
            </div>
            <select
              className="control w-36"
              value={stats.perf_tier.toUpperCase()}
              onChange={(e) => {
                const v = e.target.value;
                persist("perf_tier", v, "perf tier");
              }}
            >
              {["", "ULTRA_LOW", "LOW", "MID", "HIGH", "ULTRA"].map((t) => (
                <option key={t} value={t}>
                  {t === "" ? "— default —" : t}
                </option>
              ))}
            </select>
          </div>

          <div className="mt-3 flex items-center justify-between gap-3 border-t border-border-subtle/60 pt-3">
            <div>
              <div className="text-[12px] text-ink">Waybar layout</div>
              <div className="text-[10.5px] text-faint">
                compact (minimal) or full (all modules).
              </div>
            </div>
            <div className="flex gap-1.5">
              {(["compact", "full"] as const).map((m) => (
                <button
                  key={m}
                  className={`rounded-control border px-3 py-1.5 text-[12px] ${
                    waybarMode === m
                      ? "border-copper/60 bg-copper/10 text-ink"
                      : "border-border-subtle bg-surface-2/50 text-dim hover:border-border-strong"
                  }`}
                  onClick={() => {
                    setWaybarMode(m);
                    persist("waybar_mode", m, `waybar ${m}`);
                  }}
                >
                  {m}
                </button>
              ))}
            </div>
          </div>

          <div className="mt-3 border-t border-border-subtle/60 pt-3">
            <div className="text-[12px] text-ink">Keybinding remap</div>
            <div className="mt-1.5 flex items-center gap-2">
              <input
                className="control w-36 font-mono text-[12px]"
                placeholder="Mod4+Return"
                value={kbFrom}
                onChange={(e) => setKbFrom(e.target.value)}
              />
              <span className="text-faint">→</span>
              <input
                className="control w-36 font-mono text-[12px]"
                placeholder="3 (workspace)"
                value={kbTo}
                onChange={(e) => setKbTo(e.target.value)}
              />
              <button
                className="btn"
                disabled={!kbFrom.trim() || !kbTo.trim()}
                onClick={() => {
                  persist("keybinding", { from: kbFrom.trim(), to: kbTo.trim() }, "keybinding");
                }}
              >
                <Save size={13} /> Apply
              </button>
            </div>
            <div className="mt-1 text-[10px] text-faint">
              Writes /etc/diad/sway-overrides.conf (keysyms only: A-Z a-z 0-9 + _ : -).
            </div>
          </div>
        </div>
      </section>

      <div className="mt-5 flex items-center justify-end gap-2">
        {saved && <span className="text-[11px] text-success">✓ {saved} saved</span>}
        <button
          className="btn-ghost text-dim"
          onClick={() => toast({ kind: "info", title: "Settings", message: "Changes persist automatically via config.set.", ttl: 3000 })}
        >
          <RotateCcw size={12} /> Help
        </button>
      </div>
    </div>
  );
}
