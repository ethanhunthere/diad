import { useState } from "react";
import { FileDiff, Check, X } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import type { EditDiffCard as DiffCard } from "../lib/types";
import { basename } from "../lib/format";

/** Colorize a unified diff line by prefix. */
function DiffLine({ line }: { line: string }) {
  if (line.startsWith("+")) {
    return <div className="diff-add px-2 font-mono text-[11px] whitespace-pre">{line}</div>;
  }
  if (line.startsWith("-")) {
    return <div className="diff-del px-2 font-mono text-[11px] whitespace-pre">{line}</div>;
  }
  if (line.startsWith("@@")) {
    return <div className="bg-surface-3 px-2 font-mono text-[11px] text-copper-hover whitespace-pre">{line}</div>;
  }
  return <div className="px-2 font-mono text-[11px] text-dim whitespace-pre">{line}</div>;
}

/**
 * AI-EDIT DIFF CARD — the model proposed a file edit; the user accepts
 * or rejects, and the daemon applies it.
 */
export function EditDiffCard({ card }: { card: DiffCard }) {
  const resolveDiffCard = useStore((s) => s.resolveDiffCard);
  const toast = useStore((s) => s.toast);
  const [showDiff, setShowDiff] = useState(true);

  const accept = () => {
    resolveDiffCard(card.requestId, "accepted");
    sendMsg({ type: "edit_approve", id: card.requestId });
    toast({ kind: "success", title: "Edit accepted", message: card.path, ttl: 3000 });
  };
  const reject = () => {
    resolveDiffCard(card.requestId, "rejected");
    sendMsg({ type: "edit_reject", id: card.requestId });
  };

  const lines = card.diff.split("\n");

  return (
    <div className="overflow-hidden rounded-control border border-copper/25 bg-surface-2/50">
      <div className="flex items-center gap-2 border-b border-border-subtle px-2.5 py-1.5">
        <button
          className="flex flex-1 items-center gap-2 text-left"
          onClick={() => setShowDiff((v) => !v)}
          aria-expanded={showDiff}
        >
          <FileDiff size={13} className="text-copper-hover" />
          <span className="font-mono text-[12px] text-ink">{basename(card.path)}</span>
          <span className="truncate text-[10.5px] text-faint">{card.path}</span>
        </button>

        {card.status === "pending" && (
          <div className="flex shrink-0 items-center gap-1">
            <button
              className="flex h-5 items-center gap-1 rounded px-1.5 text-[11px] text-success hover:bg-success/15"
              onClick={accept}
              title="Accept edit"
            >
              <Check size={14} /> Accept
            </button>
            <button
              className="flex h-5 items-center gap-1 rounded px-1.5 text-[11px] text-danger hover:bg-danger/15"
              onClick={reject}
              title="Reject edit"
            >
              <X size={14} /> Reject
            </button>
          </div>
        )}
        {card.status === "accepted" && (
          <span className="flex items-center gap-1 text-[11px] text-success">
            <Check size={13} /> applied
          </span>
        )}
        {card.status === "rejected" && (
          <span className="flex items-center gap-1 text-[11px] text-danger">
            <X size={13} /> rejected
          </span>
        )}
      </div>

      {showDiff && (
        <div className="max-h-52 overflow-y-auto py-1">
          {lines.map((l, i) => (
            <DiffLine key={i} line={l} />
          ))}
        </div>
      )}
    </div>
  );
}
