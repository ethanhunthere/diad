import { useEffect, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  File as FileIcon,
  FileCode,
  FileJson,
  FileText,
  Folder,
  FolderOpen,
  Home,
  RefreshCw,
} from "lucide-react";

import { FileEntry } from "../lib/protocol";
import { useStore } from "../store";

function fileIcon(name: string, git?: string) {
  if (name.endsWith(".rs")) return <FileCode size={13.5} className="text-copper" />;
  if (name.endsWith(".ts") || name.endsWith(".tsx")) return <FileCode size={13.5} className="text-sky-400/80" />;
  if (name.endsWith(".json") || name.endsWith(".toml") || name.endsWith(".yaml") || name.endsWith(".yml"))
    return <FileJson size={13.5} className="text-warn" />;
  if (name.endsWith(".md") || name.endsWith(".txt")) return <FileText size={13.5} className="text-dim" />;
  if (git === "M" || git === "??" || git === "A" || git === "D") {
    return <FileText size={13.5} className="text-success" />;
  }
  return <FileIcon size={13.5} className="text-faint" />;
}

const GIT_STYLE: Record<string, string> = {
  M: "bg-warn/20 text-warn",
  A: "bg-success/20 text-success",
  D: "bg-danger/20 text-danger",
  "??": "bg-copper/20 text-copper",
};

function entryLabel(entry: FileEntry) {
  if (entry.kind === "dir") return <Folder size={13.5} className="shrink-0 text-warn/70" />;
  return fileIcon(entry.name, entry.git);
}

/** Left-side file tree for the Explorer view (the Ctrl+B sidebar). */
export default function Explorer() {
  const cwd = useStore((s) => s.cwd);
  const tree = useStore((s) => s.tree);
  const dirs = useStore((s) => s.dirs);
  const loadingDir = useStore((s) => s.loadingDir);
  const listDir = useStore((s) => s.listDir);
  const openTab = useStore((s) => s.openTab);
  const sidebarOpen = useStore((s) => s.sidebarOpen);
  const view = useStore((s) => s.view);

  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [pathInput, setPathInput] = useState(cwd);
  const [editingPath, setEditingPath] = useState(false);

  useEffect(() => {
    if (view === "explorer" && sidebarOpen) listDir("~");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, sidebarOpen]);

  useEffect(() => {
    setPathInput(cwd);
  }, [cwd]);

  const toggle = (path: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  const renderEntry = (entry: FileEntry, depth: number, base: string) => {
    const path = base ? `${base}/${entry.name}` : entry.name;
    const isDir = entry.kind === "dir";
    const open = !collapsed.has(path);

    return (
      <div key={path}>
        <button
          onClick={() => {
            if (isDir) {
              if (open && !(path in dirs)) listDir(path);
              toggle(path);
            } else {
              openTab(path);
            }
          }}
          className={`group flex w-full items-center gap-1.5 rounded px-1.5 py-[3px] text-left transition-colors ${
            isDir ? "text-ink/90 hover:bg-surface-2" : "text-dim hover:bg-surface-2 hover:text-ink"
          }`}
          style={{ paddingLeft: 6 + depth * 14 }}
          title={path}
        >
          {isDir ? (
            open ? (
              <ChevronDown size={12} className="shrink-0 text-faint" />
            ) : (
              <ChevronRight size={12} className="shrink-0 text-faint" />
            )
          ) : (
            <span className="w-3 shrink-0" />
          )}
          {isDir ? (
            open ? (
              <FolderOpen size={13.5} className="shrink-0 text-warn/80" />
            ) : (
              <Folder size={13.5} className="shrink-0 text-warn/70" />
            )
          ) : (
            entryLabel(entry)
          )}
          <span className="min-w-0 flex-1 truncate text-[12.5px]">{entry.name}</span>
          {entry.git && (
            <span
              className={`rounded px-1 font-mono text-[9.5px] font-bold ${GIT_STYLE[entry.git] ?? "text-dim"}`}
            >
              {entry.git}
            </span>
          )}
        </button>
        {isDir && open && (
          <div>
            {loadingDir === path ? (
              <div
                className="flex items-center gap-1.5 py-1 text-[11px] text-faint"
                style={{ paddingLeft: 20 + depth * 14 }}
              >
                <RefreshCw size={10} className="animate-spin" />
                loading…
              </div>
            ) : (
              (dirs[path] ?? []).map((e) => renderEntry(e, depth + 1, path))
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="flex h-full min-h-0 w-64 shrink-0 flex-col border-r border-border-subtle bg-surface/50">
      {/* header */}
      <div className="flex items-center gap-1 border-b border-border-subtle px-2 py-2">
        <span className="label flex-1 px-1">Explorer</span>
        <button
          className="btn-icon h-6 w-6"
          title="Home (~)"
          onClick={() => listDir("~")}
        >
          <Home size={12.5} />
        </button>
        <button className="btn-icon h-6 w-6" title="Refresh" onClick={() => listDir(cwd)}>
          <RefreshCw size={12.5} />
        </button>
      </div>

      {/* path bar */}
      <div className="px-2 py-1.5">
        {editingPath ? (
          <input
            autoFocus
            defaultValue={pathInput}
            className="input px-2 py-1 font-mono text-[11.5px]"
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                listDir((e.target as HTMLInputElement).value.trim() || "~");
                setEditingPath(false);
              }
              if (e.key === "Escape") setEditingPath(false);
            }}
            onBlur={() => setEditingPath(false)}
          />
        ) : (
          <button
            onClick={() => setEditingPath(true)}
            className="w-full truncate rounded px-1.5 py-1 text-left font-mono text-[11.5px] text-copper-dim transition-colors hover:bg-surface-2 hover:text-copper"
            title={cwd}
          >
            {cwd}
          </button>
        )}
      </div>

      {/* tree */}
      <div className="min-h-0 flex-1 overflow-y-auto px-1.5 pb-2">
        {tree.map((e) => renderEntry(e, 0, ""))}
        {!tree.length && (
          <div className="px-2 py-4 text-center text-[11.5px] text-faint">empty directory</div>
        )}
      </div>
    </div>
  );
}
