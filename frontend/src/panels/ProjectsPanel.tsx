import { useCallback, useEffect, useState } from "react";
import {
  FolderKanban,
  RefreshCw,
  FolderOpen,
  GitBranch,
  Package,
  Cpu,
  Globe,
  FileCode2,
  Loader2,
  Clock,
} from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId } from "../lib/format";
import { apiGet } from "../services/api";

interface Project {
  name: string;
  path: string;
  kind: string;
  modified: number | null;
}

const KIND_ICON: Record<string, typeof Package> = {
  git: GitBranch,
  node: Package,
  rust: Cpu,
  go: Globe,
  python: FileCode2,
  c: Cpu,
  java: Cpu,
  elixir: Globe,
  ruby: Package,
  php: Globe,
};

const KIND_COLOR: Record<string, string> = {
  git: "text-copper-hover",
  node: "text-success",
  rust: "text-warn",
  go: "text-copper-dim",
  python: "text-copper-dim",
  c: "text-dim",
  java: "text-dim",
  elixir: "text-copper-dim",
  ruby: "text-danger",
  php: "text-dim",
};

/**
 * Projects panel — scan a directory for project markers and jump into them.
 */
export function ProjectsPanel() {
  const setView = useStore((s) => s.setView);
  const setWorkspace = useStore((s) => s.setWorkspace);
  const setExplorer = useStore((s) => s.setExplorer);
  const toast = useStore((s) => s.toast);

  const [root, setRoot] = useState("~");
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(
    async (r: string) => {
      setLoading(true);
      try {
        const res = await apiGet<{ projects: Project[] }>("/projects/list", { root: r });
        setProjects(res.projects);
        setRoot(r);
      } catch (e) {
        toast({ kind: "error", title: "Projects", message: e instanceof Error ? e.message : String(e), ttl: 4000 });
      } finally {
        setLoading(false);
      }
    },
    [toast],
  );

  useEffect(() => {
    load("~");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const open = (p: Project) => {
    setWorkspace(p.path);
    setExplorer(p.path, [], true);
    sendMsg({ type: "file.list", id: nextId("fl"), path: p.path });
    setView("explorer");
    toast({ kind: "success", title: "Project opened", message: p.path, ttl: 2500 });
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1.5 border-b border-border-subtle px-3 py-2">
        <div className="flex items-center gap-1 rounded-control border border-border-subtle px-2 py-1 text-faint">
          <FolderKanban size={12} />
        </div>
        <input
          className="input h-7 min-w-0 flex-1 font-mono text-[12px]"
          value={root}
          onChange={(e) => setRoot(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && load(root)}
          placeholder="Scan directory (e.g. ~/code)"
          spellCheck={false}
        />
        <button className="btn-ghost h-7 gap-1 px-2 text-[11.5px]" onClick={() => load(root)} title="Scan">
          {loading ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />}
          Scan
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {projects.length === 0 && !loading && (
          <div className="flex flex-col items-center gap-2 py-10 text-faint">
            <FolderKanban size={22} />
            <div className="text-[12px]">No projects found in {root}</div>
          </div>
        )}

        {projects.map((p) => {
          const Icon = KIND_ICON[p.kind] ?? Package;
          return (
            <button
              key={p.path}
              onClick={() => open(p)}
              className="group flex w-full items-center gap-2.5 rounded-panel border border-border-subtle bg-surface/50 px-3 py-2 text-left transition-colors hover:border-copper/40 hover:bg-surface-2"
              title={p.path}
            >
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-control bg-surface-2">
                <Icon size={15} className={KIND_COLOR[p.kind] ?? "text-faint"} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-ink">{p.name}</div>
                <div className="truncate font-mono text-[10.5px] text-faint">{p.path}</div>
              </div>
              <div className="flex shrink-0 flex-col items-end gap-0.5">
                <span className="rounded-control bg-surface-2 px-1.5 py-0.5 font-mono text-[9.5px] uppercase text-dim">
                  {p.kind}
                </span>
                {p.modified != null && (
                  <span className="flex items-center gap-1 text-[9.5px] text-faint">
                    <Clock size={9} />
                    {new Date(p.modified * 1000).toLocaleDateString()}
                  </span>
                )}
              </div>
              <FolderOpen size={14} className="shrink-0 text-faint opacity-0 transition-opacity group-hover:opacity-100" />
            </button>
          );
        })}
      </div>
    </div>
  );
}
