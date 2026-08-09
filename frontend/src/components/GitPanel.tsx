import { useCallback, useEffect, useState } from "react";
import {
  GitBranch,
  RefreshCw,
  GitCommitHorizontal,
  FilePlus2,
  FilePen,
  Trash2,
  CircleDot,
  Loader2,
} from "lucide-react";
import { useStore } from "../store/useStore";
import { apiGet, apiPost } from "../lib/api";

interface GitStatus {
  path: string;
  branch: string;
  dirty: boolean;
  lines: string[];
}

interface GitCommit {
  short: string;
  full: string;
  author: string;
  date: string;
  subject: string;
}

const STATUS_ICON: Record<string, typeof FilePlus2> = {
  "??": FilePlus2,
  A: FilePlus2,
  M: FilePen,
  D: Trash2,
  R: FilePen,
};

const STATUS_COLOR: Record<string, string> = {
  "??": "text-copper-hover",
  A: "text-success",
  M: "text-warn",
  D: "text-danger",
  R: "text-copper-hover",
};

/**
 * Git panel — status, diff, log and commit against the sandboxed REST API.
 */
export function GitPanel() {
  const workspace = useStore((s) => s.workspace);
  const toast = useStore((s) => s.toast);

  const [path, setPath] = useState(workspace || "~");
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [diff, setDiff] = useState("");
  const [staged, setStaged] = useState(false);
  const [commits, setCommits] = useState<GitCommit[]>([]);
  const [commitMsg, setCommitMsg] = useState("");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(
    async (p: string) => {
      setLoading(true);
      try {
        const s = await apiGet<GitStatus>("/git/status", { path: p });
        setStatus(s);
        const [d, l] = await Promise.all([
          apiGet<{ diff: string }>("/git/diff", { path: p }),
          apiGet<{ commits: GitCommit[] }>("/git/log", { path: p }),
        ]);
        setDiff(d.diff);
        setCommits(l.commits);
        setPath(p);
      } catch (e) {
        setStatus(null);
        setDiff("");
        setCommits([]);
        const msg = e instanceof Error ? e.message : String(e);
        if (!msg.includes("not a git repository")) {
          toast({ kind: "error", title: "Git", message: msg, ttl: 4000 });
        }
      } finally {
        setLoading(false);
      }
    },
    [toast],
  );

  useEffect(() => {
    if (workspace && !status) load(workspace);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspace]);

  const refreshDiff = async () => {
    try {
      const d = await apiGet<{ diff: string }>("/git/diff", { path, staged });
      setDiff(d.diff);
    } catch (e) {
      toast({ kind: "error", title: "Git diff", message: e instanceof Error ? e.message : String(e), ttl: 3000 });
    }
  };

  const commit = async () => {
    if (!commitMsg.trim() || busy) return;
    setBusy(true);
    try {
      const res = await apiPost<{ committed: boolean; output: string }>("/git/commit", {
        path,
        message: commitMsg.trim(),
        confirm: true,
      });
      setCommitMsg("");
      toast({
        kind: res.committed ? "success" : "error",
        title: res.committed ? "Committed" : "Commit failed",
        message: res.output.slice(0, 300),
        ttl: 4000,
      });
      await load(path);
    } catch (e) {
      toast({ kind: "error", title: "Git commit", message: e instanceof Error ? e.message : String(e), ttl: 4000 });
    } finally {
      setBusy(false);
    }
  };

  const changed = (status?.lines ?? [])
    .filter((l) => !l.startsWith("## "))
    .map((l) => ({ code: l.slice(0, 2).trim(), file: l.slice(3) }));

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-1.5 border-b border-border-subtle px-3 py-2">
        <input
          className="input min-w-0 flex-1 font-mono text-[12px]"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && load(path)}
          placeholder="Repository path (e.g. ~/myproj)"
          spellCheck={false}
        />
        <button className="btn-ghost h-7 gap-1 px-2 text-[11.5px]" onClick={() => load(path)} title="Refresh">
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
        </button>
        {status?.branch && (
          <span className="flex items-center gap-1 rounded-control border border-border-subtle px-2 py-1 font-mono text-[11px] text-copper-hover">
            <GitBranch size={12} />
            {status.branch}
            {status.dirty && <span className="ml-1 text-warn">●</span>}
          </span>
        )}
      </div>

      {!status && !loading && (
        <div className="flex flex-1 items-center justify-center text-[12px] text-faint">
          Open a git repository to see status, diff and history.
        </div>
      )}

      {status && (
        <div className="flex min-h-0 flex-1">
          {/* Left: changed files + log */}
          <div className="flex w-64 shrink-0 flex-col overflow-hidden border-r border-border-subtle">
            <div className="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-faint">
              Changes
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-1">
              {changed.length === 0 && (
                <div className="px-2 py-1 text-[11.5px] text-faint">Working tree clean</div>
              )}
              {changed.map((c) => {
                const Icon = STATUS_ICON[c.code] ?? FilePen;
                return (
                  <button
                    key={c.file}
                    onClick={() => apiGet<{ diff: string }>("/git/diff", { path, file: c.file }).then((d) => setDiff(d.diff)).catch(() => {})}
                    className="flex w-full items-center gap-1.5 rounded-control px-2 py-1 text-left text-[12px] hover:bg-surface-2"
                    title={c.file}
                  >
                    <Icon size={12} className={STATUS_COLOR[c.code] ?? "text-faint"} />
                    <span className="truncate text-ink/90">{c.file}</span>
                    <span className="ml-auto font-mono text-[10px] text-faint">{c.code}</span>
                  </button>
                );
              })}
            </div>

            <div className="border-t border-border-subtle px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-faint">
              History
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-1">
              {commits.map((c) => (
                <div key={c.full} className="flex items-start gap-1.5 px-2 py-1">
                  <CircleDot size={11} className="mt-1 shrink-0 text-faint" />
                  <div className="min-w-0">
                    <div className="truncate text-[12px] text-ink/90" title={c.subject}>
                      {c.subject}
                    </div>
                    <div className="font-mono text-[10px] text-faint">
                      {c.short} · {c.author} · {c.date}
                    </div>
                  </div>
                </div>
              ))}
              {commits.length === 0 && (
                <div className="px-2 py-1 text-[11.5px] text-faint">No commits yet</div>
              )}
            </div>
          </div>

          {/* Right: diff + commit */}
          <div className="flex min-w-0 flex-1 flex-col">
            <div className="flex items-center gap-2 border-b border-border-subtle px-3 py-1.5">
              <button
                className={`btn-ghost h-6 px-2 text-[11px] ${staged ? "text-copper-hover" : "text-dim"}`}
                onClick={() => setStaged(!staged)}
                title="Toggle staged/unstaged"
              >
                {staged ? "Staged" : "Unstaged"}
              </button>
              <button className="btn-ghost h-6 gap-1 px-2 text-[11px] text-dim" onClick={refreshDiff}>
                <RefreshCw size={11} /> Diff
              </button>
              <div className="ml-auto flex-1" />
              <input
                className="input h-6 w-48 text-[11.5px]"
                value={commitMsg}
                onChange={(e) => setCommitMsg(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && commit()}
                placeholder="Commit message"
                spellCheck={false}
              />
              <button
                className="btn-primary h-6 gap-1 px-2 text-[11px] disabled:opacity-40"
                onClick={commit}
                disabled={!commitMsg.trim() || busy}
              >
                {busy ? <Loader2 size={11} className="animate-spin" /> : <GitCommitHorizontal size={11} />}
                Commit
              </button>
            </div>
            <pre className="min-h-0 flex-1 overflow-auto p-3 font-mono text-[11.5px] leading-relaxed text-dim">
              {diff || "(no changes)"}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
