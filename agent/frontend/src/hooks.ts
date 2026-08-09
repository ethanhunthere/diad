import { useEffect } from "react";

import { useStore } from "./store";

/**
 * Global keyboard shortcuts:
 *   Ctrl+K        command palette
 *   Ctrl+B        toggle activity sidebar
 *   Ctrl+`        terminal panel
 *   Ctrl+S        save active editor tab
 *   F6            provider picker
 *   Ctrl+Shift+X  spawn TUI client
 *   Ctrl+1..6     switch views
 */
export function useGlobalShortcuts(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();
      const mod = e.ctrlKey || e.metaKey;

      // Ctrl+K — palette
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        s.setPaletteOpen(!s.paletteOpen);
        return;
      }
      // Ctrl+B — sidebar
      if (mod && e.key.toLowerCase() === "b") {
        e.preventDefault();
        s.toggleSidebar();
        return;
      }
      // Ctrl+` — terminal
      if (mod && e.key === "`") {
        e.preventDefault();
        s.setView("terminal");
        return;
      }
      // Ctrl+S — save active tab
      if (mod && e.key.toLowerCase() === "s") {
        const tab = s.openTabs.find((t) => t.path === s.activeTab);
        if (tab && tab.dirty) {
          e.preventDefault();
          s.writeFile(tab.path, tab.content);
          s.setTabDirty(tab.path, false);
        }
        return;
      }
      // F6 — provider picker
      if (e.key === "F6") {
        e.preventDefault();
        s.setProviderPickerOpen(!s.providerPickerOpen);
        return;
      }
      // Ctrl+Shift+X — spawn TUI
      if (mod && e.shiftKey && e.key.toLowerCase() === "x") {
        e.preventDefault();
        s.spawnTui();
        return;
      }
      // Ctrl+1..6 — view switching
      if (mod && !e.shiftKey && e.key >= "1" && e.key <= "6") {
        e.preventDefault();
        const views = ["chat", "explorer", "editor", "terminal", "agents", "settings"] as const;
        s.setView(views[Number(e.key) - 1]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
