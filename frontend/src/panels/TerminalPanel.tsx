import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useStore } from "../stores/useStore";

/**
 * Terminal panel — xterm.js wired to the daemon's PTY session
 * (term_input / term_output / term_resize over the WebSocket).
 */
export function TerminalPanel() {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const send = useStore((s) => s.send);
  const setTermSize = useStore((s) => s.setTermSize);

  // Buffered term output from the store.
  const termBuffer = useStore((s) => s.termBuffer);
  // Monotonic total-bytes counter — stays correct even after the store's
  // ring buffer truncates old bytes (the old length-diff approach froze
  // forever once termBuffer saturated at 64 KB).
  const termSeq = useStore((s) => s.termSeq);

  // Last sequence value written into xterm.
  const lastSeqRef = useRef(0);

  // Create the terminal once.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: '"JetBrains Mono", ui-monospace, monospace',
      theme: {
        background: "#0F1110",
        foreground: "#E5EAE6",
        cursor: "#B32032",
        cursorAccent: "#0F1110",
        selectionBackground: "rgba(179, 32, 50, 0.3)",
        black: "#090A0A",
        red: "#B32032",
        green: "#38A169",
        yellow: "#D97706",
        blue: "#4A5568",
        magenta: "#8A968E",
        cyan: "#525D56",
        white: "#E5EAE6",
        brightBlack: "#1E2321",
        brightRed: "#D32F2F",
        brightGreen: "#48BB78",
        brightYellow: "#F59E0B",
        brightBlue: "#718096",
        brightMagenta: "#A0AEC0",
        brightCyan: "#CBD5E1",
        brightWhite: "#FFFFFF",
      },
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    fit.fit();
    term.focus();

    termRef.current = term;
    fitRef.current = fit;

    // Don't replay history that streamed before this panel mounted.
    lastSeqRef.current = useStore.getState().termSeq;

    // Send term input to the daemon PTY.
    const dataSub = term.onData((data) => {
      const s = useStore.getState();
      s.send?.({ v: 1, type: "term_input", data });
    });

    // Resize propagation.
    const reportSize = () => {
      try {
        fit.fit();
        const dims = term.cols && term.rows ? { cols: term.cols, rows: term.rows } : null;
        if (dims) useStore.getState().setTermSize(dims.cols, dims.rows);
      } catch {
        /* container hidden — ignore */
      }
    };
    reportSize();
    const ro = new ResizeObserver(() => reportSize());
    ro.observe(el);

    return () => {
      ro.disconnect();
      dataSub.dispose();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, []);

  // Pipe buffered output from the store into the terminal.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    const pending = termSeq - lastSeqRef.current;
    if (pending <= 0) return;
    // The appended delta is the LAST `pending` bytes of the buffer (the ring
    // buffer only ever drops bytes from the front). If a single chunk exceeded
    // the buffer capacity, clamp to what survived — no freeze, minor overlap.
    const incoming = termBuffer.slice(Math.max(0, termBuffer.length - pending));
    if (incoming) term.write(incoming);
    lastSeqRef.current = termSeq;
  }, [termBuffer, termSeq]);

  // Forward store size to daemon when it changes.
  useEffect(() => {
    if (send && termRef.current) {
      const t = termRef.current;
      send({ v: 1, type: "term_resize", cols: t.cols, rows: t.rows });
    }
  }, [send, setTermSize]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border-subtle bg-surface/50 px-3 py-1.5">
        <div className="flex items-center gap-2 text-[11px] text-dim">
          <span className="font-semibold text-ink">Terminal</span>
          <span className="text-faint">·</span>
          <span>daemon PTY shell</span>
        </div>
        <div className="flex items-center gap-2 text-[10px] text-faint">
          <span className="h-1.5 w-1.5 rounded-full bg-success" />
          attached
        </div>
      </div>
      <div ref={containerRef} className="min-h-0 flex-1 bg-[#0d0a08] p-1" />
    </div>
  );
}
