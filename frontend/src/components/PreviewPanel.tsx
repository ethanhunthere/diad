import { useEffect, useRef, useState } from "react";
import { RefreshCw, Eye, ExternalLink, FileCode2, Globe } from "lucide-react";
import { useStore } from "../store/useStore";
import { apiGet } from "../lib/api";

/**
 * Preview panel — live HTML preview.
 * - If the active editor file is HTML, its content is rendered via srcdoc.
 * - Otherwise any URL (default: the daemon's own GUI at 127.0.0.1:7860).
 */
export function PreviewPanel() {
  const activeFile = useStore((s) => s.activeFile);
  const openFiles = useStore((s) => s.openFiles);
  const toast = useStore((s) => s.toast);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [url, setUrl] = useState("http://127.0.0.1:7860");
  const [srcDoc, setSrcDoc] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [loading, setLoading] = useState(false);

  const active = openFiles.find((f) => f.path === activeFile);
  const isHtml = /\.(html?|svg)$/i.test(active?.path ?? "");

  // When the active file is HTML, preview it live (debounced).
  useEffect(() => {
    if (!isHtml || !active) {
      setSrcDoc(null);
      return;
    }
    const t = setTimeout(() => setSrcDoc(active.content), 250);
    return () => clearTimeout(t);
  }, [active?.content, isHtml, active]);

  const openUrl = async (u: string) => {
    setLoading(true);
    setSrcDoc(null);
    setUrl(u);
    setReloadKey((k) => k + 1);
    setLoading(false);
  };

  const loadFromFile = async (path: string) => {
    setLoading(true);
    try {
      const r = await apiGet<{ content: string }>("/files/read", { path });
      setSrcDoc(r.content);
      setUrl("");
    } catch (e) {
      toast({ kind: "error", title: "Preview", message: e instanceof Error ? e.message : String(e), ttl: 3500 });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1.5 border-b border-border-subtle px-3 py-2">
        <div className="flex items-center gap-1 rounded-control border border-border-subtle px-2 py-1 text-faint">
          <Eye size={12} />
        </div>
        <input
          className="input h-7 min-w-0 flex-1 font-mono text-[12px]"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && openUrl(url)}
          placeholder="URL or leave empty to preview the active HTML file"
          spellCheck={false}
        />
        <button className="btn-ghost h-7 gap-1 px-2 text-[11.5px]" onClick={() => openUrl(url)} title="Open URL">
          <ExternalLink size={12} /> Open
        </button>
        {isHtml && (
          <button className="btn-ghost h-7 gap-1 px-2 text-[11.5px]" onClick={() => loadFromFile(activeFile!)} title="Preview active file">
            <FileCode2 size={12} /> File
          </button>
        )}
        <button className="icon-btn" onClick={() => setReloadKey((k) => k + 1)} title="Reload">
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="flex min-h-0 flex-1 bg-white">
        {srcDoc !== null ? (
          <iframe
            key={`doc-${reloadKey}`}
            ref={iframeRef}
            className="h-full w-full border-0"
            title="Live preview"
            srcDoc={srcDoc}
            sandbox="allow-scripts"
          />
        ) : (
          <iframe
            key={`url-${reloadKey}`}
            ref={iframeRef}
            className="h-full w-full border-0"
            title="Live preview"
            src={url}
          />
        )}
        {!srcDoc && !url && (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 bg-obsidian text-faint">
            <Globe size={20} />
            <div className="text-[12px]">Enter a URL, or open an HTML file to preview it.</div>
          </div>
        )}
      </div>
    </div>
  );
}
