import { RefreshCw, Undo2, Trash2, FileX2 } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId, formatBytes, basename } from "../lib/format";

/**
 * Trash view (A7 parity: native freedesktop trash).
 *
 * Files deleted through DIAD land here (spec v1.0: $XDG_DATA_HOME/Trash)
 * instead of being permanently removed. "Restore" puts an entry back at its
 * original path; "Empty" is DESTRUCTIVE and requires an explicit click.
 */
export function TrashPanel() {
  const trash = useStore((s) => s.trash);
  const trashLoading = useStore((s) => s.trashLoading);
  const wsConnected = useStore((s) => s.wsConnected);

  const refresh = () => {
    useStore.getState().setTrashLoading(true);
    sendMsg({ type: "trash_list", id: nextId("tr") });
  };

  const restore = (name: string) => {
    sendMsg({ type: "trash_restore", id: nextId("trr"), name });
  };

  const empty = () => {
    if (!window.confirm("Permanently empty the trash? This cannot be undone.")) return;
    sendMsg({ type: "trash_empty", id: nextId("tre") });
  };

  return (
    <div className="h-full overflow-y-auto px-5 py-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-sm font-semibold text-ink">Trash</h2>
          <p className="mt-0.5 text-[12px] text-dim">
            Deleted files are recoverable until the trash is emptied.
          </p>
        </div>
        <div className="flex gap-1.5">
          <button
            className="btn-ghost h-7 gap-1 text-[11.5px]"
            onClick={refresh}
            disabled={!wsConnected || trashLoading}
            title="Refresh trash"
          >
            <RefreshCw size={13} className={trashLoading ? "animate-spin" : ""} /> Refresh
          </button>
          <button
            className="btn-danger h-7 gap-1 text-[11.5px]"
            onClick={empty}
            disabled={!wsConnected || trash.length === 0}
            title="Permanently delete everything in the trash"
          >
            <Trash2 size={13} /> Empty trash
          </button>
        </div>
      </div>

      {trash.length === 0 ? (
        <div className="panel mt-4 flex flex-col items-center gap-2 py-10 text-dim">
          <FileX2 size={20} className="text-faint" />
          <span className="text-[12.5px]">Trash is empty.</span>
        </div>
      ) : (
        <div className="mt-4 flex flex-col gap-1.5">
          {trash.map((e) => (
            <div
              key={e.name}
              className="panel flex items-center gap-3 px-3 py-2 transition-all duration-150 ease-out hover:border-copper/25"
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate font-mono text-[12px] text-ink">
                  {e.name}
                </span>
                <span
                  className="block truncate text-[10.5px] text-faint"
                  title={e.original_path}
                >
                  {e.original_path ? basename(e.original_path) : "unknown origin"} ·{" "}
                  {formatBytes(e.size)} · deleted {e.deleted_at}
                </span>
              </span>
              <button
                className="btn-primary h-6 gap-1 !py-0 text-[11px]"
                onClick={() => restore(e.name)}
                disabled={!wsConnected}
                title={`Restore ${e.name} to ${e.original_path || "its original location"}`}
              >
                <Undo2 size={11} /> Restore
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
