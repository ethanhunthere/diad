import { useEffect, useState } from "react";
import {
  Check,
  Database,
  Folder,
  KeyRound,
  Keyboard,
  Palette,
  ShieldCheck,
} from "lucide-react";

import { useStore } from "../store";

const ACCENTS = [
  { id: "copper", label: "Copper", value: "#C87F4A", hover: "#E0955C" },
  { id: "ember", label: "Ember", value: "#E07A5F", hover: "#F09477" },
  { id: "jade", label: "Jade", value: "#6FBF8B", hover: "#8AD0A2" },
  { id: "azure", label: "Azure", value: "#8FA8C8", hover: "#A8BFDB" },
];

const KEYBINDINGS = [
  { keys: "Ctrl+K", action: "Command palette" },
  { keys: "Ctrl+B", action: "Toggle Explorer sidebar" },
  { keys: "Ctrl+`", action: "Open terminal panel" },
  { keys: "Ctrl+S", action: "Save active editor tab" },
  { keys: "F6", action: "Provider picker" },
  { keys: "Ctrl+Shift+X", action: "Open TUI client" },
  { keys: "Ctrl+1 … Ctrl+6", action: "Switch views" },
];

/** Settings view: appearance, defaults, keybindings, storage, hardening. */
export default function SettingsView() {
  const provider = useStore((s) => s.provider);
  const setConfig = useStore((s) => s.setConfig);
  const toast = useStore((s) => s.toast);
  const daemonVersion = useStore((s) => s.daemonVersion);
  const stats = useStore((s) => s.stats);

  const [defaultProvider, setDefaultProvider] = useState(provider);
  const [confirmDestructive, setConfirmDestructive] = useState(true);
  const [maxHistory, setMaxHistory] = useState(200);

  useEffect(() => {
    setDefaultProvider(provider);
  }, [provider]);

  const applyTheme = (accent: string, hover: string) => {
    const root = document.documentElement;
    root.style.setProperty("--accent", accent);
    root.style.setProperty("--accent-hover", hover);
    setConfig("theme", accent);
    toast("success", "Theme accent updated");
  };

  const saveDefaults = () => {
    setConfig("default_provider", defaultProvider);
    setConfig("confirm_destructive", confirmDestructive);
    setConfig("max_history", maxHistory);
    toast("success", "Settings saved");
  };

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto max-w-3xl space-y-6 px-6 py-6">
        {/* Appearance */}
        <section className="panel p-5">
          <div className="mb-4 flex items-center gap-2">
            <Palette size={15} className="text-copper" />
            <h2 className="text-[14px] font-semibold text-ink">Appearance</h2>
          </div>
          <div className="label mb-2">Accent color</div>
          <div className="flex items-center gap-3">
            {ACCENTS.map((a) => (
              <button
                key={a.id}
                onClick={() => applyTheme(a.value, a.hover)}
                className="group flex flex-col items-center gap-1.5"
                title={a.label}
              >
                <span
                  className="flex h-9 w-9 items-center justify-center rounded-control border border-border-subtle transition-transform group-hover:scale-105"
                  style={{ background: a.value + "22" }}
                >
                  <span className="h-4 w-4 rounded-full" style={{ background: a.value }} />
                </span>
                <span className="text-[10.5px] text-dim">{a.label}</span>
              </button>
            ))}
          </div>
          <p className="mt-3 text-[11.5px] text-faint">
            Theme is dark-only by design (Copper Obsidian). The accent is stored in the daemon
            config via <code className="font-mono text-copper">config.set theme</code>.
          </p>
        </section>

        {/* Defaults */}
        <section className="panel p-5">
          <div className="mb-4 flex items-center gap-2">
            <Database size={15} className="text-copper" />
            <h2 className="text-[14px] font-semibold text-ink">Defaults</h2>
          </div>
          <div className="space-y-3">
            <div>
              <div className="label mb-1.5">Default provider</div>
              <input
                value={defaultProvider}
                onChange={(e) => setDefaultProvider(e.target.value)}
                className="input max-w-xs font-mono text-[12.5px]"
                placeholder="e.g. anthropic, openai, kimi, deepseek…"
              />
            </div>
            <label className="flex cursor-pointer items-center gap-2.5">
              <input
                type="checkbox"
                checked={confirmDestructive}
                onChange={(e) => setConfirmDestructive(e.target.checked)}
                className="h-4 w-4 accent-[#C87F4A]"
              />
              <span className="text-[13px] text-ink">Confirm destructive actions</span>
            </label>
            <div>
              <div className="label mb-1.5">Max chat history (turns)</div>
              <input
                type="number"
                min={10}
                max={2000}
                value={maxHistory}
                onChange={(e) => setMaxHistory(Number(e.target.value) || 200)}
                className="input max-w-[160px]"
              />
            </div>
            <button onClick={saveDefaults} className="btn-copper">
              <Check size={13} />
              Save defaults
            </button>
          </div>
        </section>

        {/* Keybindings */}
        <section className="panel p-5">
          <div className="mb-4 flex items-center gap-2">
            <Keyboard size={15} className="text-copper" />
            <h2 className="text-[14px] font-semibold text-ink">Keybindings</h2>
          </div>
          <div className="grid grid-cols-1 gap-1.5 md:grid-cols-2">
            {KEYBINDINGS.map((k) => (
              <div key={k.keys} className="flex items-center justify-between rounded-control bg-surface-2/50 px-3 py-2">
                <span className="font-mono text-[11.5px] text-copper">{k.keys}</span>
                <span className="text-[12.5px] text-dim">{k.action}</span>
              </div>
            ))}
          </div>
        </section>

        {/* Storage & hardening */}
        <section className="panel p-5">
          <div className="mb-4 flex items-center gap-2">
            <ShieldCheck size={15} className="text-copper" />
            <h2 className="text-[14px] font-semibold text-ink">Storage & hardening</h2>
          </div>
          <div className="space-y-2.5 text-[12.5px]">
            <div className="flex items-center gap-2">
              <Folder size={13} className="text-faint" />
              <span className="text-dim">Working directory:</span>
              <code className="min-w-0 flex-1 truncate font-mono text-copper">{stats.cwd}</code>
            </div>
            <div className="flex items-center gap-2">
              <KeyRound size={13} className="text-faint" />
              <span className="text-dim">Secrets:</span>
              <span className="font-mono text-ink/90">system keyring (keyring crate)</span>
            </div>
            <div className="flex items-center gap-2">
              <ShieldCheck size={13} className="text-faint" />
              <span className="text-dim">File sandbox:</span>
              <span className="font-mono text-ink/90">restricted to sandboxed paths</span>
            </div>
            <div className="flex items-center gap-2">
              <ShieldCheck size={13} className="text-faint" />
              <span className="text-dim">Firewall (diad-firewall):</span>
              <span className="font-mono text-ink/90">managed via chat (e.g. “block port 1337”)</span>
            </div>
            <div className="mt-3 rounded-control bg-surface-2/50 px-3 py-2 text-[11.5px] text-faint">
              DIAD AGENT daemon v{daemonVersion} · GUI protocol v1 · daemon on 127.0.0.1:7860
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
