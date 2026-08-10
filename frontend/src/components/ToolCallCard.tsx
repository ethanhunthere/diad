import { useState } from "react";
import { ChevronDown, ChevronRight, CheckCircle2, XCircle, Loader2, Wrench } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import type { ToolCallCard as ToolCard } from "../lib/types";

/**
 * Collapsible TOOL-CALL CARD — shows the tool the model wants to run,
 * with ✓ approve / ✗ reject when confirmation is required.
 */
export function ToolCallCard({ msgId, card }: { msgId: string; card: ToolCard }) {
  const toggleToolCard = useStore((s) => s.toggleToolCard);
  const resolveToolCard = useStore((s) => s.resolveToolCard);
  const [showArgs, setShowArgs] = useState(false);

  const approve = () => {
    resolveToolCard(card.requestId, "approved");
    sendMsg({ type: "tool.approve", id: card.requestId });
  };
  const reject = () => {
    resolveToolCard(card.requestId, "rejected");
    sendMsg({ type: "tool.reject", id: card.requestId });
  };

  const argsText = card.args
    ? JSON.stringify(card.args, null, 2).slice(0, 1600)
    : "";

  return (
    <div className="overflow-hidden rounded-control border border-border-subtle bg-surface-2/50">
      <div className="flex items-center gap-2 px-2.5 py-1.5">
        <button
          className="flex flex-1 items-center gap-2 text-left"
          onClick={() => toggleToolCard(msgId, card.requestId)}
          title={card.collapsed ? "Expand" : "Collapse"}
        >
          {card.collapsed ? (
            <ChevronRight size={13} className="text-faint" />
          ) : (
            <ChevronDown size={13} className="text-faint" />
          )}
          <Wrench size={13} className={card.status === "rejected" ? "text-danger" : "text-copper-hover"} />
          <span className="font-mono text-[12px] text-ink">{card.name}</span>
          <span className="text-faint">·</span>
          <span className="truncate font-mono text-[10.5px] text-dim">
            {card.callId}
          </span>
        </button>

        {/* Status / actions */}
        {card.status === "pending" && card.confirmRequired && (
          <div className="flex shrink-0 items-center gap-1">
            <button
              className="flex h-5 w-5 items-center justify-center rounded text-success hover:bg-success/15"
              title="Approve (y)"
              onClick={approve}
            >
              <CheckCircle2 size={15} />
            </button>
            <button
              className="flex h-5 w-5 items-center justify-center rounded text-danger hover:bg-danger/15"
              title="Reject (n)"
              onClick={reject}
            >
              <XCircle size={15} />
            </button>
          </div>
        )}
        {card.status === "approved" && (
          <span className="flex items-center gap-1 text-[10.5px] text-success">
            <Loader2 size={12} className="animate-spin" /> running
          </span>
        )}
        {card.status === "rejected" && (
          <span className="text-[10.5px] text-danger">✗ rejected</span>
        )}
        {card.status === "done" && (
          <span
            className={`text-[10.5px] ${card.resultSuccess === false ? "text-danger" : "text-success"}`}
          >
            {card.resultSuccess === false ? "✗ failed" : "✓ done"}
          </span>
        )}
      </div>

      {!card.collapsed && (
        <div className="border-t border-border-subtle px-2.5 py-1.5">
          {argsText && (
            <button
              className="mb-1 text-[10px] uppercase tracking-wider text-faint hover:text-dim"
              onClick={() => setShowArgs((v) => !v)}
            >
              {showArgs ? "hide args" : "show args"}
            </button>
          )}
          {showArgs && (
            <pre className="overflow-x-auto rounded bg-[#0D0A08] p-2 font-mono text-[11px] text-dim">
              {argsText}
            </pre>
          )}
          {card.resultOutput && (
            <pre className="max-h-40 overflow-y-auto whitespace-pre-wrap break-words rounded bg-[#0D0A08] p-2 font-mono text-[11px] text-ink/90">
              {card.resultOutput.slice(0, 4000)}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
