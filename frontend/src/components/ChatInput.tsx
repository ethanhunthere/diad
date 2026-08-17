import { useEffect, useRef, useState } from "react";
import { Send, CornerDownLeft } from "lucide-react";
import { useStore } from "../stores/useStore";

const SLASH_COMMANDS: { cmd: string; desc: string }[] = [
  { cmd: "/wifi", desc: "Scan & connect Wi-Fi" },
  { cmd: "/status", desc: "Hardware diagnostics" },
  { cmd: "/model", desc: "Select AI model" },
  { cmd: "/tools", desc: "List OS & coding tools" },
  { cmd: "/theme", desc: "Change appearance & colors" },
  { cmd: "/help", desc: "Show command reference" },
];

/**
 * Auto-grow chat input with comfortable 15px font, human placeholder, and responsive controls.
 */
export function ChatInput({
  onSend,
  disabled,
}: {
  onSend: (text: string) => void;
  disabled?: boolean;
}) {
  const input = useStore((s) => s.input);
  const setInput = useStore((s) => s.setInput);
  const streaming = useStore((s) => s.streaming);
  const ref = useRef<HTMLTextAreaElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuIdx, setMenuIdx] = useState(0);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [input]);

  useEffect(() => {
    ref.current?.focus();
  }, [streaming]);

  const submit = () => {
    const text = input.trim();
    if (!text) return;
    onSend(text);
    setInput("");
    setMenuOpen(false);
  };

  const handleKey = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "/" && !e.shiftKey && !e.metaKey && !e.ctrlKey && input === "") {
      setMenuOpen(true);
      setMenuIdx(0);
      return;
    }
    if (menuOpen) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMenuIdx((i) => (i + 1) % SLASH_COMMANDS.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setMenuIdx((i) => (i - 1 + SLASH_COMMANDS.length) % SLASH_COMMANDS.length);
        return;
      }
      if (e.key === "Tab" || e.key === "Enter") {
        e.preventDefault();
        const cmd = SLASH_COMMANDS[menuIdx];
        setInput(cmd.cmd);
        setMenuOpen(false);
        return;
      }
      if (e.key === "Escape") {
        setMenuOpen(false);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
      return;
    }
    if (e.key === "Escape" && input === "") {
      setMenuOpen(false);
      return;
    }
  };

  const pick = (cmd: string) => {
    setInput(cmd);
    setMenuOpen(false);
    ref.current?.focus();
  };

  return (
    <div className="relative border-t border-border-subtle/80 bg-surface/80 backdrop-blur-xl px-5 py-3.5 shadow-2xl">
      {/* "/" command menu */}
      {menuOpen && (
        <div className="glass absolute bottom-full left-5 mb-2 w-80 overflow-hidden rounded-xl border border-amber-500/40 bg-surface-2/95 shadow-2xl animate-fade-in backdrop-blur-2xl">
          <div className="border-b border-border-subtle/80 bg-surface-3/90 px-3.5 py-2 text-[11px] font-bold uppercase tracking-wider text-amber-400 font-mono">
            ✦ Quick Actions
          </div>
          <div className="p-1">
            {SLASH_COMMANDS.map((c, i) => (
              <button
                key={c.cmd}
                className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-[13px] transition-colors ${
                  i === menuIdx ? "bg-amber-500/20 text-slate-100 font-medium" : "text-slate-300 hover:text-white hover:bg-surface-3/60"
                }`}
                onMouseEnter={() => setMenuIdx(i)}
                onClick={() => pick(c.cmd)}
              >
                <span className="font-mono text-amber-400 font-semibold">{c.cmd}</span>
                <span className="flex-1 text-right text-[12px] text-slate-400">{c.desc}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="flex items-end gap-3">
        <textarea
          ref={ref}
          value={input}
          rows={1}
          disabled={disabled}
          placeholder={
            disabled
              ? "Connecting to local agent…"
              : streaming
                ? "DIAD AI is reasoning and executing tools…"
                : 'Ask anything, execute OS intents, or type "/" for commands…'
          }
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKey}
          className="min-h-[46px] flex-1 resize-none rounded-xl border border-border-subtle bg-obsidian px-4 py-2.5 text-[15px] leading-relaxed text-slate-100 placeholder:text-slate-500 focus:border-amber-500/70 focus:outline-none focus:ring-2 focus:ring-amber-500/30 disabled:opacity-60 font-sans shadow-inner"
        />
        <button
          onClick={submit}
          disabled={disabled || !input.trim()}
          className="h-[46px] w-[46px] shrink-0 flex items-center justify-center rounded-xl bg-amber-500 text-slate-950 font-bold hover:bg-amber-400 active:scale-95 disabled:opacity-30 disabled:pointer-events-none transition-all shadow-lg shadow-amber-500/20"
          title="Send intent (Enter)"
          aria-label="Send message"
        >
          <Send size={18} />
        </button>
      </div>
      <div className="mt-1.5 flex items-center justify-between text-[11px] text-slate-500 font-mono">
        <span className="flex items-center gap-1">
          <CornerDownLeft size={11} className="text-amber-400" /> Enter to send
        </span>
        <span>/ for commands · F5 terminal</span>
      </div>
    </div>
  );
}
