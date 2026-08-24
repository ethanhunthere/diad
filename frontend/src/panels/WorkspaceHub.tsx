import { useState, useEffect } from "react";
import { Globe, MessageSquare, MessagesSquare, SquareTerminal, Sparkles, Search, ExternalLink, Download } from "lucide-react";
import { DiadLogo } from "../components/DiadLogo";
import { sendMsg, useStore } from "../stores/useStore";
import { nextId } from "../lib/format";

export function WorkspaceHub() {
  const [ws, setWs] = useState<number>(1);
  const [searchQuery, setSearchQuery] = useState("");
  const [installingTelegram, setInstallingTelegram] = useState(false);
  const [installingSlack, setInstallingSlack] = useState(false);
  const toast = useStore((s) => s.toast);

  useEffect(() => {
    const path = window.location.pathname;
    const params = new URLSearchParams(window.location.search);
    if (path.includes("/workspace/1") || params.get("ws") === "1") {
      setWs(1);
      document.title = "DIAD AGENT — Workspace 1";
    } else if (path.includes("/workspace/2") || path.includes("/apps") || params.get("ws") === "2") {
      setWs(2);
      document.title = "BROWSER — Workspace 2";
    } else if (path.includes("/workspace/3") || path.includes("/launch") || params.get("ws") === "3") {
      setWs(3);
      document.title = "COMMS — Workspace 3";
    } else {
      setWs(1);
      document.title = "DIAD Desktop";
    }
  }, []);

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim()) return;
    const query = searchQuery.trim();
    if (query.startsWith("http://") || query.startsWith("https://")) {
      window.location.href = query;
    } else {
      window.location.href = `https://duckduckgo.com/?q=${encodeURIComponent(query)}`;
    }
  };

  const openUrl = (url: string) => {
    window.location.href = url;
  };

  const installTelegram = async () => {
    setInstallingTelegram(true);
    toast({ kind: "info", title: "Installing Telegram", message: "Dispatching package install intent to agent...", ttl: 4000 });
    try {
      sendMsg({
        type: "chat_send",
        id: nextId(),
        text: "Please install telegram-desktop package for me.",
      });
      setTimeout(() => {
        setInstallingTelegram(false);
        toast({ kind: "info", title: "Telegram Install Request Sent", message: "DIAD Agent is installing Telegram.", ttl: 3000 });
      }, 2000);
    } catch {
      setInstallingTelegram(false);
    }
  };

  const installSlack = async () => {
    setInstallingSlack(true);
    toast({ kind: "info", title: "Installing Slack", message: "Dispatching package install intent to agent...", ttl: 4000 });
    try {
      sendMsg({
        type: "chat_send",
        id: nextId(),
        text: "Please install slack package for me.",
      });
      setTimeout(() => {
        setInstallingSlack(false);
        toast({ kind: "info", title: "Slack Install Request Sent", message: "DIAD Agent is installing Slack.", ttl: 3000 });
      }, 2000);
    } catch {
      setInstallingSlack(false);
    }
  };

  const windowAction = async (action: "minimize" | "maximize" | "close") => {
    try {
      await fetch("/api/v1/window", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action }),
      });
    } catch {}
  };

  return (
    <div className="flex min-h-screen w-full flex-col items-center justify-center bg-[#080A09] text-white px-4 pt-12 pb-8 overflow-y-auto select-none relative">
      {/* Window Top Bar */}
      <header className="fixed top-0 left-0 right-0 h-9 bg-[#0F1311]/95 backdrop-blur-md border-b border-[#202623] flex items-center justify-between px-3 z-50">
        <div className="flex items-center gap-2 text-[12px] font-semibold text-[#CBD8D2]">
          <DiadLogo size={14} />
          <span>DIAD Workspace {ws} · {ws === 1 ? "AGENT" : ws === 2 ? "BROWSER" : "COMMS"}</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => windowAction("minimize")}
            className="flex h-6 w-6 items-center justify-center rounded text-[13px] text-[#96A9A0] hover:bg-[#18201C] hover:text-white transition-colors"
            title="Minimize"
          >
            –
          </button>
          <button
            onClick={() => windowAction("maximize")}
            className="flex h-6 w-6 items-center justify-center rounded text-[11px] text-[#96A9A0] hover:bg-[#18201C] hover:text-white transition-colors"
            title="Maximize"
          >
            □
          </button>
          <button
            onClick={() => windowAction("close")}
            className="flex h-6 w-6 items-center justify-center rounded text-[13px] text-[#96A9A0] hover:bg-[#FF2E4D]/25 hover:text-[#FF2E4D] transition-colors"
            title="Close"
          >
            ×
          </button>
        </div>
      </header>

      {/* Main Content Hub */}
      <div className="flex w-full max-w-2xl flex-col items-center gap-8 z-10 my-auto">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-[#18201C] border border-[#202623] p-3 shadow-xl">
            <DiadLogo size={40} />
          </div>
          <h1 className="text-2xl sm:text-3xl font-bold tracking-tight text-white">
            {ws === 1 && "DIAD Agent Workspace"}
            {ws === 2 && "DIAD Web & App Space"}
            {ws === 3 && "DIAD Communications"}
          </h1>
          <p className="text-[13.5px] text-[#96A9A0] max-w-md">
            {ws === 1 && "Autonomous agent core and intelligent system execution terminal."}
            {ws === 2 && "High-performance web browsing, cloud tools, and research workspace."}
            {ws === 3 && "Encrypted messaging, team collaboration, and unified comms channels."}
          </p>
        </div>
      </div>

      {/* Workspace 1: DIAD AGENT */}
      {ws === 1 && (
        <div className="flex flex-col items-center max-w-xl w-full text-center animate-fade-in my-auto">

          <div className="mt-8 grid grid-cols-1 sm:grid-cols-2 gap-4 w-full">
            <button
              onClick={() => { window.location.href = "/"; }}
              className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-amber-500/40 bg-[#151722]/90 p-5 sm:p-6 transition-all duration-200 hover:border-amber-500 hover:bg-[#1c1f2e] hover:shadow-2xl hover:shadow-amber-500/20 group cursor-pointer"
            >
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-amber-500/10 text-amber-400 group-hover:scale-110 group-hover:bg-amber-500/20 transition-all">
                <Sparkles size={26} />
              </div>
              <span className="text-[15px] font-bold text-slate-100 group-hover:text-amber-400">
                Launch DIAD Studio
              </span>
              <span className="text-[11.5px] text-slate-400 font-sans">
                Visual programming & OS controls
              </span>
            </button>

            <button
              onClick={() => { sendMsg({ type: "spawn_tui" }); }}
              className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-sky-500/40 bg-[#151722]/90 p-5 sm:p-6 transition-all duration-200 hover:border-sky-500 hover:bg-[#1c1f2e] hover:shadow-2xl hover:shadow-sky-500/20 group cursor-pointer"
            >
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-sky-500/10 text-sky-400 group-hover:scale-110 group-hover:bg-sky-500/20 transition-all">
                <SquareTerminal size={26} />
              </div>
              <span className="text-[15px] font-bold text-slate-100 group-hover:text-sky-400">
                Open Terminal (TUI)
              </span>
              <span className="text-[11.5px] text-slate-400 font-sans">
                High-speed lightweight console (F5)
              </span>
            </button>
          </div>
        </div>
      )}

      {/* Workspace 2: BROWSER */}
      {ws === 2 && (
        <div className="flex flex-col items-center max-w-xl w-full text-center animate-fade-in my-auto">
          <div className="mb-4 flex h-16 w-16 sm:h-20 sm:w-20 items-center justify-center rounded-3xl bg-cyan-500/10 border border-cyan-500/30 text-cyan-400 shadow-2xl shadow-cyan-500/20">
            <Globe size={36} />
          </div>
          <h1 className="text-lg sm:text-xl font-bold tracking-wider font-mono text-slate-100 uppercase">
            WEB BROWSER
          </h1>
          <p className="mt-1 text-[12px] sm:text-[13px] text-cyan-400/80 font-sans tracking-wide">
            Hardware-Accelerated Web Engine
          </p>

          <form onSubmit={handleSearch} className="mt-6 sm:mt-8 w-full relative">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search DuckDuckGo or enter URL..."
              className="w-full rounded-2xl border border-slate-700 bg-[#151722]/90 py-3 sm:py-3.5 pl-11 pr-12 text-[13px] sm:text-[14px] text-slate-100 placeholder-slate-500 focus:border-cyan-500 focus:outline-none focus:ring-1 focus:ring-cyan-500 transition-all shadow-inner"
            />
            <Search size={18} className="absolute left-3.5 top-3 sm:top-3.5 text-slate-400" />
            <button
              type="submit"
              className="absolute right-2.5 top-2 sm:top-2 rounded-xl bg-cyan-500/20 px-3 py-1.5 text-[12px] font-bold text-cyan-300 hover:bg-cyan-500/30 transition-all cursor-pointer"
            >
              Go
            </button>
          </form>

          <div className="mt-4 sm:mt-5 flex flex-wrap items-center justify-center gap-2">
            {[
              { label: "DuckDuckGo", url: "https://duckduckgo.com" },
              { label: "YouTube", url: "https://youtube.com" },
              { label: "GitHub", url: "https://github.com" },
              { label: "Wikipedia", url: "https://wikipedia.org" },
            ].map((link) => (
              <button
                key={link.label}
                onClick={() => openUrl(link.url)}
                className="flex items-center gap-1.5 rounded-xl border border-slate-800 bg-[#151722]/60 px-3 py-1.5 text-[12px] font-medium text-slate-300 hover:border-cyan-500/50 hover:bg-cyan-500/10 hover:text-cyan-300 transition-all cursor-pointer"
              >
                <span>{link.label}</span>
                <ExternalLink size={12} className="opacity-60" />
              </button>
            ))}
          </div>

          <div className="mt-6 sm:mt-8 w-full">
            <button
              onClick={() => openUrl("https://duckduckgo.com")}
              className="w-full flex items-center justify-center gap-2 rounded-2xl border border-cyan-500/40 bg-cyan-500/10 py-3 sm:py-3.5 text-[13px] sm:text-[14px] font-bold text-cyan-300 hover:bg-cyan-500/20 hover:border-cyan-500 transition-all cursor-pointer"
            >
              <Globe size={18} />
              Open DuckDuckGo Browser
            </button>
          </div>
        </div>
      )}

      {/* Workspace 3: COMMS (Telegram & Slack with Install Options) */}
      {ws === 3 && (
        <div className="flex flex-col items-center max-w-xl w-full text-center animate-fade-in my-auto">
          <div className="mb-4 flex h-16 w-16 sm:h-20 sm:w-20 items-center justify-center rounded-3xl bg-sky-500/10 border border-sky-500/30 text-sky-400 shadow-2xl shadow-sky-500/20">
            <MessageSquare size={36} />
          </div>
          <h1 className="text-lg sm:text-xl font-bold tracking-wider font-mono text-slate-100 uppercase">
            COMMUNICATIONS
          </h1>
          <p className="mt-1 text-[12px] sm:text-[13px] text-sky-400/80 font-sans tracking-wide">
            Official Telegram & Slack Integrations
          </p>

          <div className="mt-6 sm:mt-8 grid grid-cols-1 sm:grid-cols-2 gap-4 sm:gap-5 w-full">
            {/* Telegram Card */}
            <div className="flex flex-col items-center justify-between rounded-2xl border border-sky-500/40 bg-[#151722]/90 p-4 sm:p-5 transition-all hover:border-sky-400 hover:bg-[#1c1f2e] shadow-xl">
              <div className="flex flex-col items-center gap-2 text-center">
                <div className="flex h-12 w-12 sm:h-14 sm:w-14 items-center justify-center rounded-2xl bg-[#229ED9]/15 text-[#229ED9] shadow-lg">
                  <MessageSquare size={26} />
                </div>
                <h3 className="text-[15px] sm:text-[16px] font-bold text-slate-100">Telegram</h3>
                <p className="text-[11.5px] sm:text-[12px] text-slate-400">Fast & secure instant messenger</p>
              </div>

              <div className="mt-4 sm:mt-5 flex flex-col gap-2 w-full">
                <button
                  onClick={() => openUrl("https://web.telegram.org/a/")}
                  className="flex w-full items-center justify-center gap-2 rounded-xl bg-[#229ED9] py-2.5 text-[12.5px] sm:text-[13px] font-bold text-white hover:bg-[#1e8bc0] transition-all cursor-pointer shadow-md"
                >
                  <ExternalLink size={14} />
                  Open Web Telegram
                </button>
                <button
                  onClick={installTelegram}
                  disabled={installingTelegram}
                  className="flex w-full items-center justify-center gap-1.5 rounded-xl border border-slate-700 bg-surface-3/50 py-2 text-[11.5px] sm:text-[12px] font-medium text-slate-300 hover:border-sky-400 hover:text-sky-300 transition-all cursor-pointer"
                >
                  <Download size={13} />
                  {installingTelegram ? "Installing…" : "Install Desktop App"}
                </button>
              </div>
            </div>

            {/* Slack Card */}
            <div className="flex flex-col items-center justify-between rounded-2xl border border-purple-500/40 bg-[#151722]/90 p-4 sm:p-5 transition-all hover:border-purple-400 hover:bg-[#1c1f2e] shadow-xl">
              <div className="flex flex-col items-center gap-2 text-center">
                <div className="flex h-12 w-12 sm:h-14 sm:w-14 items-center justify-center rounded-2xl bg-[#E01E5A]/15 text-[#ECB22E] shadow-lg">
                  <MessagesSquare size={26} />
                </div>
                <h3 className="text-[15px] sm:text-[16px] font-bold text-slate-100">Slack</h3>
                <p className="text-[11.5px] sm:text-[12px] text-slate-400">Team channels, huddles & workspace</p>
              </div>

              <div className="mt-4 sm:mt-5 flex flex-col gap-2 w-full">
                <button
                  onClick={() => openUrl("https://app.slack.com/client")}
                  className="flex w-full items-center justify-center gap-2 rounded-xl bg-[#611f69] py-2.5 text-[12.5px] sm:text-[13px] font-bold text-white hover:bg-[#4a154b] transition-all cursor-pointer shadow-md"
                >
                  <ExternalLink size={14} />
                  Open Web Slack
                </button>
                <button
                  onClick={installSlack}
                  disabled={installingSlack}
                  className="flex w-full items-center justify-center gap-1.5 rounded-xl border border-slate-700 bg-surface-3/50 py-2 text-[11.5px] sm:text-[12px] font-medium text-slate-300 hover:border-purple-400 hover:text-purple-300 transition-all cursor-pointer"
                >
                  <Download size={13} />
                  {installingSlack ? "Installing…" : "Install Desktop App"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      <div className="mt-8 flex items-center gap-3 sm:gap-4 text-[11px] sm:text-[11.5px] text-slate-500 font-mono">
        <span className={ws === 1 ? "text-amber-400 font-bold" : ""}>F1 / WS1: AGENT</span>
        <span>•</span>
        <span className={ws === 2 ? "text-cyan-400 font-bold" : ""}>F2 / WS2: BROWSER</span>
        <span>•</span>
        <span className={ws === 3 ? "text-sky-400 font-bold" : ""}>F3 / WS3: COMMS</span>
      </div>

      {/* Bottom right corner resize indicator */}
      <div className="fixed bottom-2 right-2 text-slate-500 hover:text-amber-400 cursor-se-resize select-none text-[15px] font-bold z-50 p-1" title="Resize Window (Drag corner or Super+Right Click)">
        ↘
      </div>
    </div>
  );
}
