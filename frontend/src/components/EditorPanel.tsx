import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { xcodeDark } from "@uiw/codemirror-theme-xcode";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { lintGutter, linter } from "@codemirror/lint";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { keymap } from "@codemirror/view";
import type { EditorView, ViewUpdate } from "@codemirror/view";
import { X, Save, CheckCircle2, FileText, Sparkles, Wand2 } from "lucide-react";
import { useStore, sendMsg } from "../store/useStore";
import { nextId, basename } from "../lib/format";

function langFor(path: string) {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  switch (ext) {
    case "js":
    case "jsx":
    case "ts":
    case "tsx":
    case "mjs":
      return javascript({ jsx: ext === "jsx" || ext === "tsx", typescript: ext === "ts" || ext === "tsx" });
    case "py":
      return python();
    case "rs":
      return rust();
    case "json":
      return json();
    case "md":
    case "markdown":
      return markdown();
    case "html":
    case "htm":
      return html();
    case "css":
    case "scss":
      return css();
    case "sh":
    case "bash":
    case "zsh":
    case "fish":
    case "toml":
    case "yaml":
    case "yml":
    case "conf":
    case "ini":
      return undefined;
    default:
      return undefined;
  }
}

// Lightweight diagnostics: TODO/FIXME/HACK markers + unreasonably long lines.
// Real language diagnostics would need a language server; this is a useful
// dependency-free first pass that surfaces maintenance hotspots in the gutter.
const TODO_RE = /\b(TODO|FIXME|HACK|XXX)\b/;

const diagLinter = linter((view) => {
  const diagnostics: {
    from: number;
    to: number;
    severity: "warning" | "info";
    message: string;
  }[] = [];
  const doc = view.state.doc;
  for (let lineNo = 1; lineNo <= doc.lines; lineNo++) {
    const line = doc.line(lineNo);
    const m = TODO_RE.exec(line.text);
    if (m) {
      diagnostics.push({
        from: line.from + (m.index ?? 0),
        to: line.from + (m.index ?? 0) + m[0].length,
        severity: "warning",
        message: `${m[0]} marker — review before release`,
      });
    }
    if (line.text.length > 160) {
      diagnostics.push({
        from: line.from + 160,
        to: line.to,
        severity: "info",
        message: `Line is ${line.text.length} chars — consider splitting`,
      });
    }
  }
  return diagnostics;
});

/**
 * Editor panel — CodeMirror 6 with tabs, dirty markers, Ctrl+S save,
 * lint gutter, search (Ctrl+F), and AI assist on the current selection.
 */
export function EditorPanel() {
  const openFiles = useStore((s) => s.openFiles);
  const activeFile = useStore((s) => s.activeFile);
  const setActiveFile = useStore((s) => s.setActiveFile);
  const closeFile = useStore((s) => s.closeFile);
  const setFileContent = useStore((s) => s.setFileContent);
  const toast = useStore((s) => s.toast);
  const setView = useStore((s) => s.setView);

  const viewRef = useRef<EditorView | null>(null);
  const [aiPos, setAiPos] = useState<{ x: number; y: number } | null>(null);

  const active = openFiles.find((f) => f.path === activeFile);

  const save = useCallback(
    (path?: string) => {
      const target = path ?? activeFile;
      const file = openFiles.find((f) => f.path === target);
      if (!file || !file.dirty) return;
      sendMsg({ type: "file.write", id: nextId("fw"), path: file.path, content: file.content });
      toast({ kind: "info", title: "Saving…", message: basename(file.path), ttl: 1200 });
      // The daemon responds file.written → store.markFileSaved.
    },
    [activeFile, openFiles, toast],
  );

  // Ctrl+S save.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save]);

  if (openFiles.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <div className="flex h-12 w-12 items-center justify-center rounded-panel border border-border-subtle bg-surface-2/60 text-faint">
          <FileText size={20} />
        </div>
        <div className="text-sm text-dim">No file open</div>
        <div className="text-[11.5px] text-faint">
          Open a file from the Explorer, or ask the assistant to edit one.
        </div>
      </div>
    );
  }

  const onChange = useCallback(
    (value: string) => {
      if (activeFile) setFileContent(activeFile, value);
    },
    [activeFile, setFileContent],
  );

  const extensions = useMemo(() => {
    const lang = activeFile ? langFor(activeFile) : undefined;
    return [
      lang ? lang : [],
      lintGutter(),
      diagLinter,
      highlightSelectionMatches(),
      keymap.of([...searchKeymap]),
    ];
  }, [activeFile]);

  // Track the current selection so the AI assist chip can float above it.
  const onUpdate = useCallback((vu: ViewUpdate) => {
    if (!vu.selectionSet && !vu.docChanged) return;
    const view = vu.view;
    const sel = view.state.selection.main;
    if (sel.empty || view.state.doc.sliceString(sel.from, sel.to).trim() === "") {
      setAiPos(null);
      return;
    }
    const coords = view.coordsAtPos(sel.head);
    if (!coords) {
      setAiPos(null);
      return;
    }
    setAiPos({ x: coords.left, y: coords.top });
  }, []);

  const aiAssist = useCallback(
    (mode: "explain" | "fix") => {
      const view = viewRef.current;
      if (!view || !activeFile) return;
      const sel = view.state.selection.main;
      if (sel.empty) return;
      const code = view.state.doc.sliceString(sel.from, sel.to);
      const file = basename(activeFile);
      const kind = mode === "explain" ? "Explain" : "Fix";
      sendMsg({
        type: "chat.send",
        id: nextId("ai"),
        text: `${kind} the selected code in ${file}:\n\n\`\`\`\n${code}\n\`\`\``,
      });
      toast({ kind: "info", title: `AI ${kind.toLowerCase()} requested`, message: file, ttl: 1500 });
      setAiPos(null);
      setView("chat");
    },
    [activeFile, setView, toast],
  );

  return (
    <div className="flex h-full flex-col">
      {/* Tab strip */}
      <div className="flex items-center border-b border-border-subtle bg-surface/50">
        <div className="flex min-w-0 flex-1 items-center overflow-x-auto">
          {openFiles.map((f) => {
            const isActive = f.path === activeFile;
            return (
              <div
                key={f.path}
                onClick={() => setActiveFile(f.path)}
                className={`group flex max-w-[200px] cursor-pointer items-center gap-1.5 border-r border-border-subtle px-3 py-1.5 text-[12px] ${
                  isActive
                    ? "bg-obsidian text-ink"
                    : "text-dim hover:bg-surface-2"
                }`}
                title={f.path}
              >
                <span className="truncate">{basename(f.path)}</span>
                {f.dirty ? (
                  <span className="h-1.5 w-1.5 rounded-full bg-copper" title="Unsaved" />
                ) : (
                  <CheckCircle2 size={11} className="text-faint" />
                )}
                <button
                  className="ml-0.5 text-faint opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
                  onClick={(e) => {
                    e.stopPropagation();
                    closeFile(f.path);
                  }}
                  title="Close"
                >
                  <X size={12} />
                </button>
              </div>
            );
          })}
        </div>
        <div className="flex items-center gap-1 pr-2">
          {active?.dirty && (
            <button className="btn-ghost h-6 gap-1 px-2 text-[11px] text-copper-hover" onClick={() => save()}>
              <Save size={12} /> Save
            </button>
          )}
        </div>
      </div>

      {/* CodeMirror */}
      <div className="relative min-h-0 flex-1">
        <CodeMirror
          key={activeFile ?? "none"}
          value={active?.content ?? ""}
          onChange={onChange}
          extensions={extensions}
          onCreateEditor={(view) => {
            viewRef.current = view;
          }}
          onUpdate={onUpdate}
          theme={xcodeDark}
          height="100%"
          style={{ height: "100%" }}
          basicSetup={{
            lineNumbers: true,
            foldGutter: true,
            highlightActiveLine: true,
            highlightActiveLineGutter: true,
            autocompletion: true,
            bracketMatching: true,
            closeBrackets: true,
            indentOnInput: true,
          }}
        />

        {/* AI assist chip — floats above the current selection */}
        {aiPos && (
          <div
            className="absolute z-30 flex items-center gap-1 rounded-full border border-copper/50 bg-surface px-2 py-1 shadow-lg"
            style={{ left: aiPos.x + 8, top: aiPos.y - 12 }}
          >
            <Sparkles size={12} className="text-copper-hover" />
            <button
              className="flex items-center gap-1 text-[11px] font-medium text-ink hover:text-copper-hover"
              onClick={() => aiAssist("explain")}
              title="Ask the assistant to explain the selection"
            >
              Explain
            </button>
            <span className="h-3 w-px bg-border-subtle" />
            <button
              className="flex items-center gap-1 text-[11px] font-medium text-ink hover:text-copper-hover"
              onClick={() => aiAssist("fix")}
              title="Ask the assistant to fix the selection"
            >
              <Wand2 size={11} /> Fix
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
