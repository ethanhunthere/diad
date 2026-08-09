import { useMemo, useState } from "react";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Loader2,
  TerminalSquare,
  Wrench,
  X,
} from "lucide-react";

import { EditDiffCard, ToolCallCard } from "../lib/protocol";
import { useStore } from "../store";

// ---------------------------------------------------------------------------
// Tool call card — collapsible glass card with args + live output
// ---------------------------------------------------------------------------

export function ToolCallCardView({ card }: { card: ToolCallCard }) {
  const [open, setOpen] = useState(false);
  const approve = useStore((s) => s.approveTool);
  const reject = useStore((s) => s.rejectTool);
  const argsText = useMemo(() => JSON.stringify(card.args, null, 2), [card.args]);
  const shortArgs = useMemo(() => {
    const t = argsText.replace(/\s+/g, " ");
    return t.length > 90 ? t.slice(0, 87) + "…" : t;
  }, [argsText]);

  const statusIcon = card.pending ? (
    <Loader2 size={13} className="animate-spin text-copper" />
  ) : card.success ? (
    <Check size={13} className="text-success" />
  ) : (
    <X size={13} className="text-danger" />
  );

  return (
    <div className="glass my-2 overflow-hidden rounded-control">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-surface-2/60"
      >
        {open ? (
          <ChevronDown size={13} className="shrink-0 text-faint" />
        ) : (
          <ChevronRight size={13} className="shrink-0 text-faint" />
        )}
        <Wrench size={13} className="shrink-0 text-copper" />
        <span className="font-mono text-[12.5px] font-semibold text-ink">{card.name}</span>
        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-faint">
          {shortArgs}
        </span>
        {card.confirmRequired && !card.pending && (
          <span className="shrink-0 rounded bg-warn/15 px-1.5 py-0.5 text-[10px] font-semibold text-warn">
            requires approval
          </span>
        )}
        {statusIcon}
      </button>

      {open && (
        <div className="border-t border-border-subtle px-3 py-2">
          <pre className="overflow-x-auto rounded bg-obsidian/60 p-2 font-mono text-[11.5px] leading-relaxed text-dim">
            {argsText}
          </pre>
          {card.output !== undefined && (
            <pre className="mt-2 max-h-56 overflow-y-auto rounded bg-obsidian/60 p-2 font-mono text-[11.5px] leading-relaxed text-ink/80">
              {card.output || "(no output)"}
            </pre>
          )}
        </div>
      )}

      {card.confirmRequired && card.pending && (
        <div className="flex items-center gap-2 border-t border-border-subtle px-3 py-2">
          <span className="text-[11.5px] text-warn">This tool modifies the system.</span>
          <div className="flex-1" />
          <button
            onClick={() => reject(card.callId)}
            className="rounded-control border border-border-subtle px-2.5 py-1 text-[11.5px] font-semibold text-dim transition-colors hover:border-danger/60 hover:text-danger"
          >
            Reject
          </button>
          <button
            onClick={() => approve(card.callId)}
            className="rounded-control bg-copper px-2.5 py-1 text-[11.5px] font-semibold text-[#1a0f07] transition-colors hover:bg-copper-hover"
          >
            Approve
          </button>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Edit diff card — accept/reject a proposed file edit
// ---------------------------------------------------------------------------

function DiffLine({ line }: { line: string }) {
  if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("@@")) {
    return (
      <div className="bg-copper/10 px-2 font-semibold text-copper-dim">{line}</div>
    );
  }
  if (line.startsWith("+")) {
    return <div className="bg-success/10 px-2 text-success/90">{line}</div>;
  }
  if (line.startsWith("-")) {
    return <div className="bg-danger/10 px-2 text-danger/90">{line}</div>;
  }
  return <div className="px-2 text-dim">{line}</div>;
}

export function EditDiffCardView({ card }: { card: EditDiffCard }) {
  const approve = useStore((s) => s.approveEdit);
  const reject = useStore((s) => s.rejectEdit);
  const lines = useMemo(() => card.diff.split("\n"), [card.diff]);

  return (
    <div className="glass my-2 overflow-hidden rounded-control">
      <div className="flex items-center gap-2 border-b border-border-subtle px-3 py-2">
        <TerminalSquare size={13} className="shrink-0 text-copper" />
        <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-ink" title={card.path}>
          {card.path}
        </span>
        {card.resolved && (
          <span
            className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${
              card.accepted
                ? "bg-success/15 text-success"
                : "bg-danger/15 text-danger"
            }`}
          >
            {card.accepted ? "applied" : "rejected"}
          </span>
        )}
      </div>

      <div className="max-h-64 overflow-y-auto py-1 font-mono text-[11.5px] leading-[1.6]">
        {lines.map((l, i) => (
          <DiffLine key={i} line={l} />
        ))}
      </div>

      {!card.resolved && (
        <div className="flex items-center gap-2 border-t border-border-subtle px-3 py-2">
          <span className="text-[11.5px] text-dim">Apply this edit?</span>
          <div className="flex-1" />
          <button
            onClick={() => reject(card.id)}
            className="rounded-control border border-border-subtle px-2.5 py-1 text-[11.5px] font-semibold text-dim transition-colors hover:border-danger/60 hover:text-danger"
          >
            Reject
          </button>
          <button
            onClick={() => approve(card.id)}
            className="rounded-control bg-copper px-2.5 py-1 text-[11.5px] font-semibold text-[#1a0f07] transition-colors hover:bg-copper-hover"
          >
            Accept
          </button>
        </div>
      )}
    </div>
  );
}
