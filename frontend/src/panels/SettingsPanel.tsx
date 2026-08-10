import { useEffect, useState } from "react";
import { Save, RotateCcw, Palette, Folder, Shield, KeyRound } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";

const THEMES = [
  { id: "copper", label: "Copper Obsidian", swatch: "#C87F4A" },
  { id: "emerald", label: "Emerald Night", swatch: "#6FBF8B" },
  { id: "azure", label: "Azure Dusk", swatch: "#7AA7D9" },
  { id: "violet", label: "Violet Void", swatch: "#9E8BC4" },
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
  const models = useStore((s) => s.models);
  const toast = useStore((s) => s.toast);

  const [saved, setSaved] = useState<string | null>(null);

  const persist = (key: string, value: unknown, label: string) => {
    sendMsg({ type: "config_set", key, value });
    setSaved(label);
    setTimeout(() => setSaved(null), 1500);
  };

  const changeAccent = (id: string) => {
    setSettings({ theme: id });
    document.documentElement.style.setProperty("--accent", THEMES.find((t) => t.id === id)?.swatch ?? "#C87F4A");
    persist("theme", id, "theme");
  };

  // Load stored accent on mount.
  useEffect(() => {
    const t = THEMES.find((x) => x.id === settings.theme)?.swatch ?? "#C87F4A";
    document.documentElement.style.setProperty("--accent", t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="h-full overflow-y-auto px-5 py-4">
      <h2 className="text-sm font-semibold text-ink">Settings</h2>
      <p className="mt-0.5 text-[12px] text-dim">
        Preferences are stored by the daemon and survive restarts.
      </p>

      {/* Appearance */}
      <section className="mt-5">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <Palette size={12} /> Appearance
        </div>
        <div className="panel p-3">
          <div className="text-[11.5px] text-dim">Accent theme</div>
          <div className="mt-2 flex flex-wrap gap-2">
            {THEMES.map((t) => (
              <button
                key={t.id}
                className={`flex items-center gap-2 rounded-control border px-3 py-1.5 text-[12px] transition-all duration-150 ease-out ${
                  settings.theme === t.id
                    ? "border-copper/60 bg-copper/10 text-ink"
                    : "border-border-subtle bg-surface-2/50 text-dim hover:border-border-strong"
                }`}
                onClick={() => changeAccent(t.id)}
              >
                <span
                  className="h-3 w-3 rounded-full"
                  style={{ background: t.swatch }}
                />
                {t.label}
              </button>
            ))}
          </div>
        </div>
      </section>

      {/* Default provider */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <KeyRound size={12} /> Default provider
        </div>
        <div className="panel p-3">
          <select
            className="control w-full"
            value={settings.defaultProvider}
            disabled={!wsConnected}
            onChange={(e) => {
              const v = e.target.value;
              setSettings({ defaultProvider: v });
              persist("default_provider", v, "default provider");
            }}
          >
            <option value="">— default —</option>
            {providers.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>
          {models.length > 0 && (
            <div className="mt-2 text-[10.5px] text-faint">
              {models.length} models available for the current provider
            </div>
          )}
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
