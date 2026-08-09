import { useCallback, useEffect, useMemo, useRef } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { javascript } from "@codemirror/lang-javascript";
import { rust } from "@codemirror/lang-rust";
import { json } from "@codemirror/lang-json";
import { python } from "@codemirror/lang-python";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { sql } from "@codemirror/lang-sql";
import { oneDark } from "@codemirror/theme-one-dark";
import { X, FileCode2 } from "lucide-react";

import { useStore } from "../store";

function langFor(path: string) {
  if (/\.(md|markdown)$/.test(path)) return markdown({ base: markdownLanguage });
  if (/\.(ts|tsx|js|jsx|mjs|cjs)$/.test(path)) return javascript({ jsx: true, typescript: true });
  if (/\.rs$/.test(path)) return rust();
  if (/\.json$/.test(path)) return json();
  if (/\.py$/.test(path)) return python();
  if (/\.(html|htm|svg)$/.test(path)) return html();
  if (/\\.(css|scss|sass)$/.test(path)) return css();
  if (/\.sql$/.test(path)) return sql();
  return [];
}

/** Code editor with tab strip (Ctrl+S saves the active tab). */
export default function Editor() {
  const openTabs = useStore((s) => s.openTabs);
  const activeTab = useStore((s) => s.activeTab);
  const closeTab = useStore((s) => s.closeTab);
  const writeFile = useStore((s) => s.writeFile);
  const setTabDirty = useStore((s) => s.setTabDirty);

  const active = useMemo(
    () => openTabs.find((t) => t.path === activeTab) ?? null,
    [openTabs, activeTab]
  );
  const savedRef = useRef<string | null>(null);

  // Track the last saved content to mark dirty correctly.
  useEffect(() => {
    if (active) savedRef.current = active.content;
  }, [activeTab]);

  const onChange = useCallback(
    (value: string) => {
      if (!active) return;
      useStore.setState((s) => ({
        openTabs: s.openTabs.map((t) =>
          t.path === active.path ? { ...t, content: value, dirty: value !== savedRef.current } : t
        ),
      }));
    },
    [active]
  );

  const onSave = useCallback(() => {
    if (!active) return;
    writeFile(active.path, active.content);
    savedRef.current = active.content;
    setTabDirty(active.path, false);
  }, [active, writeFile, setTabDirty]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        onSave();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onSave]);

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      {/* Tab strip */}
      <div className="flex shrink-0 items-stretch overflow-x-auto border-b border-border-subtle bg-surface/60">
        {openTabs.map((t) => (
          <button
            key={t.path}
            onClick={() => useStore.setState({ activeTab: t.path })}
            className={`group flex shrink-0 items-center gap-1.5 border-r border-border-subtle px-3 py-1.5 text-[12px] transition-colors ${
              t.path === activeTab
                ? "bg-obsidian/40 text-ink"
                : "text-dim hover:bg-surface-2 hover:text-ink"
            }`}
            title={t.path}
          >
            <FileCode2 size={12} className="text-copper-dim" />
            <span className="max-w-[180px] truncate font-mono">
              {t.path.split("/").pop()}
            </span>
            {t.dirty && <span className="h-1.5 w-1.5 rounded-full bg-copper" />}
            <span
              role="button"
              tabIndex={0}
              onClick={(e) => {
                e.stopPropagation();
                closeTab(t.path);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") closeTab(t.path);
              }}
              className="rounded p-0.5 text-faint opacity-0 transition-opacity hover:text-danger group-hover:opacity-100"
            >
              <X size={11} />
            </span>
          </button>
        ))}
        {!openTabs.length && (
          <div className="px-3 py-1.5 text-[12px] text-faint">
            No files open — pick one from the Explorer.
          </div>
        )}
      </div>

      {/* Editor body */}
      <div className="min-h-0 flex-1 overflow-hidden">
        {active ? (
          <CodeMirror
            value={active.content}
            onChange={onChange}
            extensions={[langFor(active.path), oneDark]}
            theme="dark"
            basicSetup={{
              lineNumbers: true,
              foldGutter: true,
              highlightActiveLine: true,
              highlightActiveLineGutter: true,
              autocompletion: true,
              bracketMatching: true,
              closeBrackets: true,
              indentOnInput: true,
              tabSize: 2,
            }}
            style={{ height: "100%" }}
          />
        ) : (
          <div className="flex h-full items-center justify-center">
            <div className="text-center text-faint">
              <FileCode2 size={36} className="mx-auto mb-3 opacity-40" strokeWidth={1.2} />
              <p className="text-[13px]">No file selected</p>
              <p className="mt-1 text-[11.5px]">
                Open a file from the Explorer, or ask DIAD AGENT to edit one.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
