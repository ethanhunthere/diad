interface DiadLogoProps {
  size?: number;
  className?: string;
  showSubtitle?: boolean;
  variant?: "white" | "black";
}

/**
 * DIAD — Official wordmark logo with signature red square dot on the 'i'.
 */
export function DiadLogo({
  size = 18,
  className = "",
  showSubtitle = false,
  variant = "white",
}: DiadLogoProps) {
  const logoSrc = variant === "black" ? "/diad-logo-black.png" : "/diad-logo-white.png";

  return (
    <div className={`inline-flex items-center gap-2 select-none ${className}`}>
      <img
        src={logoSrc}
        alt="DiAD"
        style={{ height: `${size}px`, width: "auto" }}
        className="object-contain"
        draggable={false}
      />
      {showSubtitle && (
        <span className="text-[11px] font-semibold tracking-wider text-slate-400 font-sans uppercase">
          Agent
        </span>
      )}
    </div>
  );
}
