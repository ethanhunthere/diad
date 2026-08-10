import { useCallback, useEffect, useRef, useState } from "react";
import {
  Play,
  Pause,
  ChevronDown,
  ChevronRight,
  CornerDownRight,
  SkipForward,
  Square,
  Plus,
  Trash2,
  Bug,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { useStore } from "../stores/useStore";
import { apiGet, apiPost } from "../services/api";

interface BreakpointDto {
  number: number;
  file: string;
  line: number;
  enabled: boolean;
}

interface FrameDto {
  level: number;
  func: string;
  file: string;
  line: number;
}

interface VarDto {
  name: string;
  value: string;
  kind: string;
}

interface DebugState {
  session: string;
  program: string;
  running: boolean;
  exited: boolean;
  exit_code: number | null;
  reason: string | null;
  breakpoints: BreakpointDto[];
  stack: FrameDto[];
  variables: VarDto[];
  console: string[];
}

interface SessionInfo {
  session: string;
  program: string;
}

/**
 * Debug panel — GDB (MI-2) sessions served by the daemon's /debug REST API.
 */
export function DebugPanel() {
  const toast = useStore((s) => s.toast);
  const [program, setProgram] = useState("");
  const [args, setArgs] = useState("");
  const [cwd, setCwd] = useState("");
  const [session, setSession] = useState<string | null>(null);
  const [snap, setSnap] = useState<DebugState | null>(null);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [bpFile, setBpFile] = useState("");
  const [bpLine, setBpLine] = useState("");
  const [starting, setStarting] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const consoleRef = useRef<HTMLDivElement>(null);

  const refreshSessions = useCallback(async () => {
    try {
      const r = await apiGet<{ sessions: SessionInfo[] }>("/debug/sessions");
      setSessions(r.sessions);
    } catch {
      /* daemon offline — ignore */
    }
  }, []);

  useEffect(() => {
    refreshSessions();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fetchState = useCallback(async (id: string) => {
    try {
      const s = await apiPost<DebugState>("/debug/state", { session: id });
      setSnap(s);
    } catch {
      setSnap(null);
    }
  }, []);

  const start = async () => {
    if (!program.trim() || starting) return;
    setStarting(true);
    try {
      const r = await apiPost<{ session: string; program: string }>("/debug/session", {
        program: program.trim(),
        args: args.trim() ? args.trim().split(/\s+/) : [],
        working_dir: cwd.trim() || null,
      });
      setSession(r.session);
      await fetchState(r.session);
      await refreshSessions();
      toast({ kind: "success", title: "Debug session started", message: r.program, ttl: 2500 });
    } catch (e) {
      toast({ kind: "error", title: "Debug", message: e instanceof Error ? e.message : String(e), ttl: 4000 });
    } finally {
      setStarting(false);
    }
  };

  const stop = async () => {
    if (!session) return;
    await apiPost<{ stopped: boolean }>("/debug/stop", { session }).catch(() => {});
    setSession(null);
    setSnap(null);
    await refreshSessions();
  };

  const control = async (action: "run" | "continue" | "step" | "next" | "finish") => {
    if (!session) return;
    try {
      await apiPost<{ ok: boolean }>(`/debug/${action}`, { session });
      // Give gdb a moment to report the stop, then refresh.
      setTimeout(() => fetchState(session), 400);
    } catch (e) {
      toast({ kind: "error", title: `debug/${action}`, message: e instanceof Error ? e.message : String(e), ttl: 3500 });
    }
  };

  const addBreakpoint = async () => {
    if (!session || !bpFile.trim() || !bpLine.trim()) return;
    try {
      await apiPost<{ breakpoint: BreakpointDto }>("/debug/breakpoint", {
        session,
        file: bpFile.trim(),
        line: parseInt(bpLine, 10),
      });
      setBpLine("");
      await fetchState(session);
    } catch (e) {
      toast({ kind: "error", title: "Breakpoint", message: e instanceof Error ? e.message : String(e), ttl: 3500 });
    }
  };

  const removeBreakpoint = async (number: number) => {
    if (!session) return;
    await apiPost<{ deleted: boolean }>("/debug/delete-breakpoint", { session, number }).catch(() => {});
    await fetchState(session);
  };

  const refresh = async () => {
    if (!session || refreshing) return;
    setRefreshing(true);
    try {
      await apiPost<{ stack: FrameDto[]; variables: VarDto[] }>("/debug/frames", { session }).catch(() => {});
      await fetchState(session);
    } finally {
      setRefreshing(false);
    }
  };

  // Poll while a session is active so stepping/stopping is reflected.
  useEffect(() => {
    if (!session) return;
    const t = setInterval(() => fetchState(session), 2000);
    return () => clearInterval(t);
  }, [session, fetchState]);

  useEffect(() => {
    consoleRef.current?.scrollTo({ top: consoleRef.current.scrollHeight });
  }, [snap?.console]);

  const running = snap?.running ?? false;
  const exited = snap?.exited ?? false;

  return (
    <div className="flex h-full flex-col">
      {/* Session config */}
      <div className="flex flex-wrap items-center gap-1.5 border-b border-border-subtle px-3 py-2">
        <input
          className="input h-7 w-44 font-mono text-[12px]"
          value={program}
          onChange={(e) => setProgram(e.target.value)}
          placeholder="Program (e.g. ~/myproj/target/debug/app)"
          spellCheck={false}
        />
        <input
          className="input h-7 w-40 font-mono text-[12px]"
          value={args}
          onChange={(e) => setArgs(e.target.value)}
          placeholder="Arguments"
          spellCheck={false}
        />
        <input
          className="input h-7 w-40 font-mono text-[12px]"
          value={cwd}
          onChange={(e) => setCwd(e.target.value)}
          placeholder="Working dir (optional)"
          spellCheck={false}
        />
        <button className="btn-primary h-7 gap-1 px-2.5 text-[11.5px]" onClick={start} disabled={!program.trim() || starting}>
          {starting ? <Loader2 size={12} className="animate-spin" /> : <Bug size={12} />}
          Start
        </button>

        {sessions.length > 0 && (
          <select
            className="input h-7 text-[11.5px]"
            value={session ?? ""}
            onChange={(e) => {
              const id = e.target.value;
              setSession(id || null);
              if (id) fetchState(id);
            }}
          >
            <option value="">— sessions —</option>
            {sessions.map((s) => (
              <option key={s.session} value={s.session}>
                {s.session} ({s.program})
              </option>
            ))}
          </select>
        )}

        {session && (
          <button className="btn-ghost h-7 gap-1 px-2 text-[11.5px] text-danger" onClick={stop}>
            <Square size={11} /> Stop
          </button>
        )}
        {session && (
          <span className="ml-auto font-mono text-[10.5px] text-faint">
            {exited ? `exited (${snap?.exit_code ?? "?"})` : running ? "● running" : "■ stopped"}
          </span>
        )}
      </div>

      {!session && (
        <div className="flex flex-1 items-center justify-center text-[12px] text-faint">
          Start a session to inspect breakpoints, stack and variables.
        </div>
      )}

      {session && snap && (
        <div className="flex min-h-0 flex-1">
          {/* Left: breakpoints + controls */}
          <div className="flex w-60 shrink-0 flex-col overflow-hidden border-r border-border-subtle">
            <div className="flex items-center gap-1 border-b border-border-subtle px-2 py-1.5">
              <div className="flex-1 text-[10px] font-semibold uppercase tracking-wider text-faint">
                Breakpoints
              </div>
              <button className="icon-btn" onClick={refresh} title="Refresh state">
                <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
              </button>
            </div>
            <div className="flex gap-1 border-b border-border-subtle px-2 py-1.5">
              <input
                className="input h-6 min-w-0 flex-1 font-mono text-[11px]"
                value={bpFile}
                onChange={(e) => setBpFile(e.target.value)}
                placeholder="file.rs"
                spellCheck={false}
              />
              <input
                className="input h-6 w-12 font-mono text-[11px]"
                value={bpLine}
                onChange={(e) => setBpLine(e.target.value)}
                placeholder="10"
                spellCheck={false}
              />
              <button className="icon-btn" onClick={addBreakpoint} title="Add breakpoint">
                <Plus size={13} />
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-1">
              {snap.breakpoints.length === 0 && (
                <div className="px-2 py-1 text-[11.5px] text-faint">No breakpoints</div>
              )}
              {snap.breakpoints.map((b) => (
                <div key={b.number} className="group flex items-center gap-1.5 rounded-control px-2 py-1 text-[12px]">
                  <span className="h-2 w-2 rounded-full bg-danger" />
                  <span className="truncate font-mono text-ink/90">
                    {b.file}:{b.line}
                  </span>
                  <button
                    className="ml-auto text-faint opacity-0 group-hover:opacity-100 hover:text-danger"
                    onClick={() => removeBreakpoint(b.number)}
                    title={`Delete breakpoint ${b.number}`}
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
            </div>

            {/* Controls */}
            <div className="grid grid-cols-3 gap-1 border-t border-border-subtle p-2">
              <button className="btn-ghost h-8 flex-col gap-0.5 text-[10px]" onClick={() => control("run")} disabled={running} title="Run">
                <Play size={13} /> Run
              </button>
              <button className="btn-ghost h-8 flex-col gap-0.5 text-[10px]" onClick={() => control("continue")} title="Continue">
                <Pause size={13} /> Continue
              </button>
              <button className="btn-ghost h-8 flex-col gap-0.5 text-[10px]" onClick={() => control("step")} title="Step into">
                <ChevronDown size={13} /> Step
              </button>
              <button className="btn-ghost h-8 flex-col gap-0.5 text-[10px]" onClick={() => control("next")} title="Step over">
                <CornerDownRight size={13} /> Next
              </button>
              <button className="btn-ghost h-8 flex-col gap-0.5 text-[10px]" onClick={() => control("finish")} title="Step out">
                <SkipForward size={13} /> Finish
              </button>
              <button className="btn-ghost h-8 flex-col gap-0.5 text-[10px] text-danger" onClick={stop} title="Kill">
                <Square size={13} /> Kill
              </button>
            </div>
          </div>

          {/* Middle: stack + variables */}
          <div className="flex w-64 shrink-0 flex-col overflow-hidden border-r border-border-subtle">
            <div className="border-b border-border-subtle px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-faint">
              Stack {snap.reason ? `· ${snap.reason}` : ""}
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-1">
              {snap.stack.length === 0 && (
                <div className="px-2 py-1 text-[11.5px] text-faint">Not running</div>
              )}
              {snap.stack.map((f) => (
                <div key={f.level} className="flex items-center gap-1.5 rounded-control px-2 py-1 text-[12px]">
                  <ChevronRight size={12} className="shrink-0 text-faint" />
                  <div className="min-w-0">
                    <div className="truncate text-ink/90">{f.func}</div>
                    <div className="truncate font-mono text-[10.5px] text-faint">
                      {f.file}:{f.line}
                    </div>
                  </div>
                </div>
              ))}
            </div>

            <div className="border-y border-border-subtle px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-faint">
              Variables
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-1">
              {snap.variables.length === 0 && (
                <div className="px-2 py-1 text-[11.5px] text-faint">None</div>
              )}
              {snap.variables.map((v, i) => (
                <div key={`${v.name}-${i}`} className="flex items-baseline gap-1.5 rounded-control px-2 py-0.5 font-mono text-[11.5px]">
                  <span className="text-copper-dim">{v.name}</span>
                  <span className="text-faint">=</span>
                  <span className="min-w-0 truncate text-ink/90">{v.value}</span>
                  <span className="ml-auto shrink-0 text-[9.5px] text-faint">{v.kind}</span>
                </div>
              ))}
            </div>
          </div>

          {/* Right: gdb console */}
          <div className="flex min-w-0 flex-1 flex-col">
            <div className="border-b border-border-subtle px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-faint">
              gdb console
            </div>
            <div ref={consoleRef} className="min-h-0 flex-1 overflow-auto bg-obsidian p-3 font-mono text-[11.5px] leading-relaxed text-dim">
              {snap.console.length === 0 && <span className="text-faint">(gdb)</span>}
              {snap.console.map((l, i) => (
                <div key={i} className="whitespace-pre-wrap">{l}</div>
              ))}
              <div className="text-copper-dim">(gdb)</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
