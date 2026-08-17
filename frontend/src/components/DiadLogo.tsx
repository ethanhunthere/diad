interface DiadLogoProps {
  size?: number;
  className?: string;
  showWordmark?: boolean;
}

export function DiadLogo({ size = 28, className = "", showWordmark = true }: DiadLogoProps) {
  return (
    <div className={`flex items-center gap-2.5 select-none ${className}`}>
      {/* 2040 Quantum Prism Geometric Icon */}
      <svg
        width={size}
        height={size}
        viewBox="0 0 32 32"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="shrink-0 drop-shadow-[0_0_10px_rgba(245,158,11,0.35)] transition-transform hover:scale-105 duration-200"
      >
        <defs>
          <linearGradient id="diad-grad-primary" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#fbbf24" />
            <stop offset="50%" stopColor="#f59e0b" />
            <stop offset="100%" stopColor="#d97706" />
          </linearGradient>
          <linearGradient id="diad-grad-cyan" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#38bdf8" />
            <stop offset="100%" stopColor="#0284c7" />
          </linearGradient>
        </defs>

        {/* Outer Titanium Facet */}
        <polygon
          points="16,2 30,9 30,23 16,30 2,23 2,9"
          stroke="url(#diad-grad-primary)"
          strokeWidth="1.8"
          fill="rgba(13, 14, 21, 0.85)"
          strokeLinejoin="round"
        />

        {/* Inner Dual Intelligence Nodes */}
        <polygon
          points="16,6 25,11 25,21 16,26 7,21 7,11"
          fill="rgba(245, 158, 11, 0.12)"
          stroke="rgba(245, 158, 11, 0.4)"
          strokeWidth="1"
          strokeLinejoin="round"
        />

        {/* Dynamic Quantum Core */}
        <circle cx="16" cy="16" r="3.2" fill="url(#diad-grad-primary)" />
        <circle cx="16" cy="16" r="1.2" fill="#ffffff" />

        {/* Telemetry Wave Accent */}
        <path
          d="M10 16H13L15 13L17 19L19 16H22"
          stroke="url(#diad-grad-cyan)"
          strokeWidth="1.2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>

      {showWordmark && (
        <div className="flex flex-col justify-center">
          <div className="flex items-center gap-1.5 leading-none">
            <span className="text-[15px] font-black tracking-widest text-slate-100 uppercase font-mono">
              DIAD
            </span>
            <span className="text-[9px] font-bold px-1.5 py-0.5 rounded-full bg-amber-500/20 text-amber-400 border border-amber-500/40 uppercase tracking-wider">
              2040
            </span>
          </div>
          <span className="text-[10px] font-medium tracking-wider text-slate-400 uppercase mt-0.5">
            Autonomous OS
          </span>
        </div>
      )}
    </div>
  );
}
