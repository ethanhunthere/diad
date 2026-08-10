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

          {/* Font scale (spec 2.9): 0.8x–2.0x, live, persisted. */}
          <div className="mt-4 text-[11.5px] text-dim">
            Font scale — <span className="font-mono text-ink">{settings.fontScale.toFixed(2)}x</span>
          </div>
          <input
            type="range"
            min={0.8}
            max={2}
            step={0.05}
            value={settings.fontScale}
            onChange={(e) => setSettings({ fontScale: parseFloat(e.target.value) })}
            className="mt-1 w-full accent-[#C87F4A]"
          />

          {/* Density override (spec 2.7) */}
          <div className="mt-3 text-[11.5px] text-dim">Layout density</div>
          <div className="mt-1.5 flex gap-2">
            {(["auto", "compact", "comfortable"] as const).map((d) => (
              <button
                key={d}
                className={`rounded-control border px-3 py-1.5 text-[12px] ${
                  settings.density === d
                    ? "border-copper/60 bg-copper/10 text-ink"
                    : "border-border-subtle bg-surface-2/50 text-dim hover:border-border-strong"
                }`}
                onClick={() => setSettings({ density: d })}
              >
                {d}
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
