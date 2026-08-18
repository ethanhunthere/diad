interface DiadLogoProps {
  size?: number;
  className?: string;
  showSubtitle?: boolean;
}

/**
 * DIAD — Minimalist, modern typography wordmark.
 * Designed with refined tracking and neo-grotesque clarity.
 */
export function DiadLogo({ size, className = "", showSubtitle = false }: DiadLogoProps) {
  return (
    <div className={`inline-flex items-center gap-2 select-none ${className}`}>
      <span
        className="font-sans font-bold tracking-[0.2em] text-slate-100 uppercase transition-colors hover:text-white"
        style={{ fontSize: size ? `${size}px` : "14px" }}
      >
        DIAD
      </span>
      {showSubtitle && (
        <span className="text-[11px] font-medium tracking-wider text-slate-400 font-sans">
          Agent
        </span>
      )}
    </div>
  );
}
