import { useEffect } from "react";
import { useStore, sendMsg } from "../stores/useStore";

/**
 * Global keybindings (DIAD AGENT spec):
 *   Ctrl+K   → command palette
 *   Ctrl+B   → toggle activity bar
 *   Ctrl+`   → terminal
 *   Ctrl+S   → save (handled in EditorPanel)
 *   F6       → provider picker (palette scoped to providers)
 *   Ctrl+Shift+X → spawn TUI (daemon opens foot)
 */
export function useGlobalKeys(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();

      // Don't hijack keys while typing in an input/textarea/codemirror
      // (except the shortcuts we own).
      const target = e.target as HTMLElement | null;
      const isTyping =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable ||
          target.closest(".cm-editor") !== null);

      // Ctrl+K — palette (even while typing).
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        s.setPaletteOpen(!s.paletteOpen);
        return;
      }

      // Ctrl+B — toggle activity bar (even while typing).
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
        e.preventDefault();
        s.toggleActivityBar();
        return;
      }

      // F6 — provider picker (palette pre-scoped to providers).
      if (e.key === "F6") {
        e.preventDefault();
        s.setPaletteOpen(true);
        return;
      }

      // Ctrl+Shift+X — spawn TUI via the daemon.
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "x") {
        e.preventDefault();
        sendMsg({ type: "spawn.tui" });
        return;
      }

      if (isTyping) return;

      // Ctrl+` — terminal (when not typing).
      if ((e.ctrlKey || e.metaKey) && e.key === "`") {
        e.preventDefault();
        s.setView("terminal");
        return;
      }
    };

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
