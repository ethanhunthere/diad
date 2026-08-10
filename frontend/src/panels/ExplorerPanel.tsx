import { useEffect, useState } from "react";
import {
  Folder,
  File,
  ArrowUp,
  GitBranch,
  RefreshCw,
  Home,
  Trash2,
} from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId, formatBytes } from "../lib/format";
import type { FileEntryDto } from "../lib/types";

const GIT_STYLE: Record<string, string> = {
  M: "text-warn",
  A: "text-success",
  D: "text-danger",
  R: "text-copper-hover",
  "??": "text-copper-hover",
};

const HOME = "~";

/**
 * Explorer panel — git-aware file browser talking to the daemon's
 * file.list (single level, sandboxed to allowed paths).
 */
export function ExplorerPanel() {
  const path = useStore((s) => s.explorerPath) || HOME;
  const entries = useStore((s) => s.explorerEntries);
  const loading = useStore((s) => s.explorerLoading);
  const setExplorer = useStore((s) => s.setExplorer);
  const toast = useStore((s) => s.toast);

  const [err, setErr] = useState<string | null>(null);

  const list = (p: string) => {
    setErr(null);
    setExplorer(p, [], true);
    sendMsg({ type: "file_list", id: nextId("fl"), path: p });
  };

  useEffect(() => {
    list(HOME);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const goUp = () => {
    const resolved = path === HOME ? "~" : path;
    const idx = resolved.lastIndexOf("/");
    if (idx <= 0) {
      list(HOME);
    } else {
      list(resolved.slice(0, idx));
    }
  };

  const open = (e: FileEntryDto) => {
    const full = path === HOME ? `~/${e.name}` : `${path}/${e.name}`;
    if (e.kind === "dir") {
      list(full);
    } else if (e.kind === "symlink") {
      toast({ kind: "warn", title: "Symlink", message: "Opening symlinks is not supported.", ttl: 3000 });
    } else {
      sendMsg({ type: "file_read", id: nextId("fr"), path: full });
    }
  };

  // A7 parity: delete now moves the file to the freedesktop trash (the
  // daemon expands `~` and re-checks the allowed-path sandbox).
  const trash = (e: FileEntryDto) => {
    const full = path === HOME ? `~/${e.name}` : `${path}/${e.name}`;
    if (!window.confirm(`Move ${e.name} to trash?`)) return;
    sendMsg({ type: "trash_move", id: nextId("tm"), path: full });
    // Refresh the listing; the daemon ack re-syncs the Trash panel.
    list(path);
  };

  const gitBadges = entries.filter((e) => e.git && e.git !== "✓");

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1.5 border-b border-border-subtle px-3 py-2">
        <button className="icon-btn" onClick={() => list(HOME)} title="Home (~)">
          <Home size={14} />
        </button>
        <button className="icon-btn" onClick={goUp} title="Up">
          <ArrowUp size={14} />
        </button>
        <button
          className="icon-btn"
          onClick={() => list(path)}
          title="Refresh"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
        <div className="ml-1 flex-1 truncate font-mono text-[11.5px] text-dim" title={path}>
          {path}
        </div>
        {gitBadges.length > 0 && (
          <span className="flex items-center gap-1 text-[10px] text-faint">
            <GitBranch size={11} />
            {gitBadges.length}
          </span>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {err && <div className="px-2 py-1 text-[12px] text-danger">{err}</div>}

        {loading && entries.length === 0 && (
          <div className="space-y-1.5 px-2 py-2">
            {Array.from({ length: 8 }, (_, i) => {
              // F-17: key by the deterministic width value, never the index —
              // index-as-key mis-associates DOM across re-renders.
              const w = 60 + ((i * 37) % 35);
              return (
                <div
                  key={w}
                  className="h-6 animate-pulse rounded bg-surface-2"
                  style={{ width: `${w}%` }}
                />
              );
            })}
          </div>
        )}

        {!loading && entries.length === 0 && !err && (
          <div className="px-2 py-4 text-center text-[11.5px] text-faint">
            Empty directory
          </div>
        )}

        <div className="space-y-0.5">
          {entries.map((e) => {
            const isDir = e.kind === "dir";
            const badge = e.git && e.git !== "✓" ? e.git : null;
            return (
              <div
                key={e.name}
                onClick={() => open(e)}
                role="button"
                tabIndex={0}
                onKeyDown={(ev) => {
                  if (ev.key === "Enter" || ev.key === " ") {
                    ev.preventDefault();
                    open(e);
                  }
                }}
                className="group flex w-full cursor-pointer items-center gap-2 rounded-control px-2 py-1 text-left text-[12.5px] hover:bg-surface-2 transition-colors duration-100"
                title={`${isDir ? "Directory" : "File"} — ${e.name}`}
              >
                {isDir ? (
                  <Folder size={14} className="shrink-0 text-copper-dim" />
                ) : (
                  <File size={14} className="shrink-0 text-faint" />
                )}
                <span className="flex-1 truncate text-ink/90">{e.name}</span>
                {badge && (
                  <span
                    className={`font-mono text-[10px] ${GIT_STYLE[badge] ?? "text-faint"}`}
                    title={`git status: ${badge}`}
                  >
                    {badge}
                  </span>
                )}
                {!isDir && e.size > 0 && (
                  <span className="shrink-0 font-mono text-[10px] text-faint">
                    {formatBytes(e.size)}
                  </span>
                )}
                {!isDir && (
                  <button
                    className="icon-btn hidden h-6 w-6 group-hover:inline-flex"
                    onClick={(ev) => {
                      ev.stopPropagation();
                      trash(e);
                    }}
                    title={`Move ${e.name} to trash`}
                  >
                    <Trash2 size={13} />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
