import { useEffect, useRef, useState } from "react";
import { Send, CornerDownLeft } from "lucide-react";
import { useStore } from "../stores/useStore";

const SLASH_COMMANDS: { cmd: string; desc: string }[] = [
  { cmd: "/help", desc: "Show help" },
  { cmd: "/tools", desc: "List available tools" },
  { cmd: "/version", desc: "Show DIAD AGENT version" },
  { cmd: "/model", desc: "Pick a model" },
  { cmd: "/connect", desc: "Connect a provider" },
  { cmd: "/wifi", desc: "Connect to Wi-Fi" },
];

/**
 * Auto-grow chat input. Enter sends, Shift+Enter inserts a newline,
 * "/" opens the command menu.
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

  // Auto-grow.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
  }, [input]);

  // Focus on mount and when streaming finishes.
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
    // "/" command menu
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
    <div className="relative border-t border-border-subtle bg-surface/40 px-3 py-2.5">
      {/* "/" command menu */}
      {menuOpen && (
        <div className="glass absolute bottom-full left-3 mb-2 w-72 overflow-hidden rounded-panel shadow-xl animate-fade-in">
          <div className="border-b border-border-subtle px-3 py-1.5 text-[10px] uppercase tracking-wider text-faint">
            Commands
          </div>
          {SLASH_COMMANDS.map((c, i) => (
            <button
              key={c.cmd}
              className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12px] ${
                i === menuIdx ? "bg-copper/10 text-copper-hover" : "text-dim"
              }`}
              onMouseEnter={() => setMenuIdx(i)}
              onClick={() => pick(c.cmd)}
            >
              <span className="font-mono">{c.cmd}</span>
              <span className="flex-1 text-right text-[10.5px] text-faint">{c.desc}</span>
            </button>
          ))}
        </div>
      )}

      <div className="flex items-end gap-2">
        <textarea
          ref={ref}
          value={input}
          rows={1}
          disabled={disabled}
          placeholder={
            disabled
              ? "Daemon offline — starting…"
              : streaming
                ? "Assistant is streaming…"
                : 'Ask DIAD AGENT anything…  ("/" for commands)'
          }
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKey}
          className="min-h-[38px] flex-1 resize-none rounded-panel border border-border-subtle bg-obsidian/80 px-3 py-2 text-[13px] text-ink placeholder:text-faint focus:border-copper/60 focus:outline-none focus:ring-1 focus:ring-copper/30 disabled:opacity-60"
        />
        <button
          onClick={submit}
          disabled={disabled || !input.trim()}
          className="btn-primary h-[38px] w-[38px] shrink-0 justify-center !px-0 disabled:opacity-40"
          title="Send (Enter)"
        >
          <Send size={15} />
        </button>
      </div>
      <div className="mt-1 flex items-center gap-2 text-[10px] text-faint">
        <span>
          <CornerDownLeft size={9} className="inline" /> Enter send
        </span>
        <span>·</span>
        <span>Shift+Enter newline</span>
        {!disabled && (
          <>
            <span className="ml-auto">
              Ctrl+K palette · F6 provider · Ctrl+` terminal
            </span>
          </>
        )}
      </div>
    </div>
  );
}
