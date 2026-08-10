import { useEffect } from "react";
import { ArrowUpRight, LayoutGrid, SquareTerminal, Globe } from "lucide-react";
import { sendMsg } from "../stores/useStore";

/**
 * DIAD Launchpad (MASTER FIX S4/S5) — minimal app launcher.
 *
 * Served by the daemon's SPA fallback at `/launch` and opened on workspace 3
 * as `brave --app=http://127.0.0.1:7860/launch`. It sets `document.title` to
 * "DIAD Launchpad" so sway's `assign [title="DIAD Launchpad"] 3` rule (added
 * in S5, before the browser rule) pins it to workspace 3.
 *
 * It is intentionally tiny: a browser app-window cannot exec native
 * binaries, so launching happens through the daemon WebSocket
 * (`spawn_tui`) or by navigating the current window.
 */
export function Launchpad() {
  useEffect(() => {
    document.title = "DIAD Launchpad";
  }, []);

  const openAgent = () => {
    window.location.href = "/";
  };
  const openTerminal = () => {
    sendMsg({ type: "spawn_tui" });
  };
  const openBrowser = () => {
    window.open("/", "_blank");
  };

  return (
    <div className="flex h-full flex-col items-center justify-center bg-obsidian text-ink">
      <div className="mb-6 flex h-16 w-16 items-center justify-center rounded-panel border border-copper/40 bg-copper/15 text-4xl text-copper-hover">
        ◈
      </div>
      <h1 className="text-xl font-semibold tracking-wide">DIAD Launchpad</h1>
      <p className="mt-2 text-[12.5px] text-dim">Intent is the only interface.</p>

      <div className="mt-10 grid grid-cols-3 gap-4">
        <button
          className="flex w-36 flex-col items-center gap-2 rounded-panel border border-border-subtle bg-surface-2/50 px-4 py-5 text-[12.5px] transition-all duration-150 ease-out hover:border-copper/40 hover:bg-surface-2"
          onClick={openAgent}
          title="Open the DIAD AGENT GUI"
        >
          <LayoutGrid size={22} className="text-copper-hover" />
          DIAD AGENT
          <ArrowUpRight size={12} className="text-faint" />
        </button>
        <button
          className="flex w-36 flex-col items-center gap-2 rounded-panel border border-border-subtle bg-surface-2/50 px-4 py-5 text-[12.5px] transition-all duration-150 ease-out hover:border-copper/40 hover:bg-surface-2"
          onClick={openTerminal}
          title="Open the terminal client (daemon spawns foot)"
        >
          <SquareTerminal size={22} className="text-copper-hover" />
          Terminal
          <ArrowUpRight size={12} className="text-faint" />
        </button>
        <button
          className="flex w-36 flex-col items-center gap-2 rounded-panel border border-border-subtle bg-surface-2/50 px-4 py-5 text-[12.5px] transition-all duration-150 ease-out hover:border-copper/40 hover:bg-surface-2"
          onClick={openBrowser}
          title="Open a new browser window"
        >
          <Globe size={22} className="text-copper-hover" />
          Browser
          <ArrowUpRight size={12} className="text-faint" />
        </button>
      </div>

      <p className="mt-10 text-[10.5px] text-faint">
        127.0.0.1:7860 · loopback only
      </p>
    </div>
  );
}
