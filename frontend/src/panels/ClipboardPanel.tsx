import { RefreshCw, Copy, Eraser } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId } from "../lib/format";

/**
 * Clipboard History view (A7 parity: cliphist).
 *
 * Lists the wayland clipboard history; "Use" decodes the selected entry and
 * writes it back to the clipboard, "Clear" wipes the whole history. Every
 * mutation refreshes the list via the daemon's `clipboard_list` response.
 */
export function ClipboardPanel() {
  const clipboard = useStore((s) => s.clipboard);
  const wsConnected = useStore((s) => s.wsConnected);

  const refresh = () => {
    sendMsg({ type: "clipboard_list", id: nextId("cb") });
  };

  const pick = (index: number) => {
    sendMsg({ type: "clipboard_pick", id: nextId("cbp"), index });
  };

  const clear = () => {
    sendMsg({ type: "clipboard_clear", id: nextId("cbc") });
  };

  return (
    <div className="h-full overflow-y-auto px-5 py-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-ink">Clipboard History</h2>
          <p className="mt-0.5 text-[12px] text-dim">
            cliphist-backed wayland clipboard. Click “Use” to restore an entry.
          </p>
        </div>
        <div className="flex gap-1.5">
          <button
            className="btn-ghost h-7 gap-1 text-[11.5px]"
            onClick={refresh}
            disabled={!wsConnected}
            title="Refresh clipboard history"
          >
            <RefreshCw size={13} /> Refresh
          </button>
          <button
            className="btn-danger h-7 gap-1 text-[11.5px]"
            onClick={clear}
            disabled={!wsConnected || clipboard.length === 0}
            title="Clear the entire clipboard history"
          >
            <Eraser size={13} /> Clear
          </button>
        </div>
      </div>

      {clipboard.length === 0 ? (
        <div className="panel mt-4 flex flex-col items-center gap-2 py-10 text-dim">
          <Copy size={20} className="text-faint" />
          <span className="text-[12.5px]">Clipboard history is empty.</span>
        </div>
      ) : (
        <div className="mt-4 flex flex-col gap-1.5">
          {clipboard.map((e) => (
            <div
              key={e.index}
              className="panel flex items-center gap-3 px-3 py-2 transition-all duration-150 ease-out hover:border-copper/25"
            >
              <span className="w-8 shrink-0 font-mono text-[11px] text-faint">
                #{e.index}
              </span>
              <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-ink">
                {e.preview}
              </span>
              <button
                className="btn-primary h-6 gap-1 !py-0 text-[11px]"
                onClick={() => pick(e.index)}
                disabled={!wsConnected}
                title={`Restore clipboard entry #${e.index}`}
              >
                <Copy size={11} /> Use
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
