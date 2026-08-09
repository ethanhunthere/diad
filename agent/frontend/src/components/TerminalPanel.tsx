import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";

import { daemon } from "../lib/ws";
import { useStore } from "../store";

/**
 * Live PTY terminal attached to the daemon (spawned by the DIAD daemon at
 * startup). Output streams in via `term.output`; keystrokes go out via
 * `term.input`; the size is pushed on every fit via `term.resize`.
 */
export default function TerminalPanel() {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const term = new Terminal({
      fontFamily: '"JetBrains Mono", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 1.35,
      cursorBlink: true,
      cursorStyle: "bar",
      theme: {
        background: "#0B0908",
        foreground: "#EDE6DE",
        cursor: "#C87F4A",
        selectionBackground: "rgba(200, 127, 74, 0.28)",
        black: "#1B1511",
        red: "#E07A7A",
        green: "#6FBF8B",
        yellow: "#E0B060",
        blue: "#8FA8C8",
        magenta: "#C89BC8",
        cyan: "#7FC4C0",
        white: "#EDE6DE",
        brightBlack: "#5C534A",
        brightRed: "#E58A8A",
        brightGreen: "#8AD0A2",
        brightYellow: "#E8C078",
        brightBlue: "#A8BFDB",
        brightMagenta: "#D4B0D4",
        brightCyan: "#98D6D2",
        brightWhite: "#F5F0EA",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    fit.fit();

    termRef.current = term;
    fitRef.current = fit;
    term.focus();

    // Incoming daemon output
    const off = daemon.onMessage((msg) => {
      if (msg.type === "term.output") term.write(msg.data);
    });

    // Keystrokes → daemon
    const onData = (data: string) => {
      useStore.getState().termInput(data);
    };
    term.onData(onData);

    // Size sync
    const pushSize = () => {
      fit.fit();
      useStore.getState().termResize(term.cols, term.rows);
    };
    pushSize();
    const ro = new ResizeObserver(() => pushSize());
    ro.observe(host);

    // Return focus whenever the panel becomes visible again
    const focusTimer = window.setTimeout(() => term.focus(), 60);

    return () => {
      ro.disconnect();
      window.clearTimeout(focusTimer);
      off();
      term.dispose();
      termRef.current = null;
    };
  }, []);

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col bg-obsidian">
      <div className="flex shrink-0 items-center gap-2 border-b border-border-subtle bg-surface/60 px-3 py-1.5">
        <span className="label">Terminal</span>
        <span className="text-[11px] text-faint">— DIAD daemon PTY ($SHELL)</span>
        <div className="flex-1" />
        <span className="flex items-center gap-1.5 text-[11px] text-success">
          <span className="h-1.5 w-1.5 rounded-full bg-success" />
          live
        </span>
      </div>
      <div ref={hostRef} className="min-h-0 flex-1 overflow-hidden px-2 pb-2 pt-1" />
    </div>
  );
}
