import { useEffect } from "react";
import { LayoutGrid, SquareTerminal, Globe, MessageSquare, MessagesSquare, Wifi } from "lucide-react";
import { sendMsg } from "../stores/useStore";
import { DiadLogo } from "../components/DiadLogo";

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
    window.open("https://duckduckgo.com", "_blank");
  };
  const openTelegram = () => {
    window.open("https://web.telegram.org/a/", "_blank");
  };
  const openSlack = () => {
    window.open("https://app.slack.com/client", "_blank");
  };

  return (
    <div className="flex h-full flex-col items-center justify-center bg-obsidian text-slate-100 p-6">
      <div className="mb-4 flex flex-col items-center">
        <DiadLogo size={32} />
      </div>
      <p className="mt-1 text-[13px] text-slate-400 font-sans">Intent-Driven Operating System · Workspace Navigation</p>

      <div className="mt-8 grid grid-cols-2 sm:grid-cols-3 gap-4 max-w-xl w-full">
        <button
          className="flex flex-col items-center gap-2.5 rounded-2xl border border-border-subtle bg-surface-2/80 p-5 text-[13px] font-semibold transition-all duration-200 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 group"
          onClick={openAgent}
          title="Open the DIAD AI Studio"
        >
          <LayoutGrid size={24} className="text-amber-400 group-hover:scale-110 transition-transform" />
          <span className="text-slate-200 group-hover:text-amber-400">DIAD Studio</span>
          <span className="text-[11px] text-slate-500 font-normal">Workspace 1</span>
        </button>

        <button
          className="flex flex-col items-center gap-2.5 rounded-2xl border border-border-subtle bg-surface-2/80 p-5 text-[13px] font-semibold transition-all duration-200 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 group"
          onClick={openTerminal}
          title="Open TUI Terminal"
        >
          <SquareTerminal size={24} className="text-sky-400 group-hover:scale-110 transition-transform" />
          <span className="text-slate-200 group-hover:text-sky-400">Terminal</span>
          <span className="text-[11px] text-slate-500 font-normal">F5 / foot</span>
        </button>

        <button
          className="flex flex-col items-center gap-2.5 rounded-2xl border border-border-subtle bg-surface-2/80 p-5 text-[13px] font-semibold transition-all duration-200 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 group"
          onClick={openBrowser}
          title="Open Web Browser"
        >
          <Globe size={24} className="text-emerald-400 group-hover:scale-110 transition-transform" />
          <span className="text-slate-200 group-hover:text-emerald-400">Browser</span>
          <span className="text-[11px] text-slate-500 font-normal">Workspace 2</span>
        </button>

        <button
          className="flex flex-col items-center gap-2.5 rounded-2xl border border-border-subtle bg-surface-2/80 p-5 text-[13px] font-semibold transition-all duration-200 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 group"
          onClick={openTelegram}
          title="Open Telegram Web App"
        >
          <MessageSquare size={24} className="text-sky-400 group-hover:scale-110 transition-transform" />
          <span className="text-slate-200 group-hover:text-sky-400">Telegram</span>
          <span className="text-[11px] text-slate-500 font-normal">Workspace 3</span>
        </button>

        <button
          className="flex flex-col items-center gap-2.5 rounded-2xl border border-border-subtle bg-surface-2/80 p-5 text-[13px] font-semibold transition-all duration-200 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 group"
          onClick={openSlack}
          title="Open Slack App"
        >
          <MessagesSquare size={24} className="text-purple-400 group-hover:scale-110 transition-transform" />
          <span className="text-slate-200 group-hover:text-purple-400">Slack</span>
          <span className="text-[11px] text-slate-500 font-normal">Workspace 3</span>
        </button>

        <button
          className="flex flex-col items-center gap-2.5 rounded-2xl border border-border-subtle bg-surface-2/80 p-5 text-[13px] font-semibold transition-all duration-200 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 group"
          onClick={() => { window.location.href = "/?view=settings"; }}
          title="System Settings & Wi-Fi"
        >
          <Wifi size={24} className="text-amber-400 group-hover:scale-110 transition-transform" />
          <span className="text-slate-200 group-hover:text-amber-400">Wi-Fi & Settings</span>
          <span className="text-[11px] text-slate-500 font-normal">System Config</span>
        </button>
      </div>

      <p className="mt-10 text-[11px] text-slate-500 font-mono">
        DIAD Sovereign OS
      </p>
    </div>
  );
}
