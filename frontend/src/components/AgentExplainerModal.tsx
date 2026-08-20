import { useState, useEffect } from "react";
import { ChevronLeft, ChevronRight, X, Cpu, Terminal, Wrench } from "lucide-react";

interface Slide {
  badge: string;
  title: string;
  subtitle: string;
  icon: typeof Cpu;
  points: { title: string; desc: string }[];
}

const SLIDES: Slide[] = [
  {
    badge: "01 / ARCHITECTURE",
    title: "Local Agent Daemon Core",
    subtitle: "Autonomous Rust execution engine running locally at 127.0.0.1:7860",
    icon: Cpu,
    points: [
      {
        title: "Native JSON-RPC Daemon",
        desc: "Speaks protocol v1 over local WebSocket with live streaming tokens, tool approvals, and system state.",
      },
      {
        title: "100% Private & Local",
        desc: "Your credentials remain in your local system keyring. Zero telemetry or external telemetry relays.",
      },
      {
        title: "Direct System Integration",
        desc: "Embedded in DIAD OS with Sway Wayland IPC, Foot terminal launcher, and direct kernel event listeners.",
      },
    ],
  },
  {
    badge: "02 / NATIVE TOOLS",
    title: "Real OS & Execution Tools",
    subtitle: "Direct capabilities implemented in the DIAD Agent Rust codebase",
    icon: Wrench,
    points: [
      {
        title: "Shell & Process Engine",
        desc: "Runs bash commands with real-time stdout/stderr streaming, timeout safety, and interrupt handling.",
      },
      {
        title: "Native Filesystem & Diffs",
        desc: "File reader, atomic writer with interactive syntax diff review, directory trees, and search.",
      },
      {
        title: "Desktop & Multi-Model Router",
        desc: "Freedesktop Trash, cliphist clipboard, Wi-Fi nmcli, and unified routing for Claude, Gemini, OpenAI, Ollama & DeepSeek.",
      },
    ],
  },
  {
    badge: "03 / WORKFLOW",
    title: "Dual Interface Parity & Vision",
    subtitle: "Seamless workflows across graphical studio and terminal client",
    icon: Terminal,
    points: [
      {
        title: "Instant GUI ↔ TUI Switching",
        desc: "Press F5 or click TUI anytime to split or switch between Web Studio and the high-speed Ratatui terminal.",
      },
      {
        title: "Multimodal Image Analysis",
        desc: "Drag, paste, or upload images directly into the chatbox for vision-enabled LLM reasoning and bug detection.",
      },
      {
        title: "Persistent Intent Sessions",
        desc: "Shared session state across F1 (Agent Studio), F2 (Web Browser), and F3 (Comms Launchpad).",
      },
    ],
  },
];

export function AgentExplainerModal({
  onClose,
}: {
  onClose?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [currentSlide, setCurrentSlide] = useState(0);

  useEffect(() => {
    const dismissed = localStorage.getItem("diad.explainer_dismissed");
    if (!dismissed) {
      setOpen(true);
    }
  }, []);

  const closeTour = () => {
    setOpen(false);
    localStorage.setItem("diad.explainer_dismissed", "true");
    if (onClose) onClose();
  };

  const nextSlide = () => {
    if (currentSlide < SLIDES.length - 1) {
      setCurrentSlide((s) => s + 1);
    } else {
      closeTour();
    }
  };

  const prevSlide = () => {
    if (currentSlide > 0) {
      setCurrentSlide((s) => s - 1);
    }
  };

  if (!open) return null;

  const slide = SLIDES[currentSlide];
  const Icon = slide.icon;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4 backdrop-blur-xl animate-fade-in select-none">
      <div className="relative w-full max-w-xl overflow-hidden rounded-2xl border border-[#202623] bg-[#161918] p-6 sm:p-8 shadow-2xl animate-modal-in">
        {/* Top Header */}
        <div className="flex items-center justify-between border-b border-[#202623] pb-4">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-[#1E2321] text-[#B32032] border border-[#202623]">
              <Icon size={18} />
            </div>
            <div>
              <span className="text-[11px] font-bold uppercase tracking-widest text-[#B32032] font-mono">
                {slide.badge}
              </span>
              <h2 className="text-[17px] font-bold text-[#E5EAE6] leading-tight">
                {slide.title}
              </h2>
            </div>
          </div>
          <button
            onClick={closeTour}
            className="flex h-7 w-7 items-center justify-center rounded-lg text-[#8A968E] hover:bg-[#1E2321] hover:text-[#E5EAE6] transition-colors"
            title="Close"
          >
            <X size={16} />
          </button>
        </div>

        {/* Slide Content */}
        <div className="py-5">
          <p className="text-[13px] text-[#8A968E] mb-4">
            {slide.subtitle}
          </p>

          <div className="space-y-3">
            {slide.points.map((pt, i) => (
              <div
                key={i}
                className="rounded-xl border border-[#202623] bg-[#0F1110] p-3.5 hover-elevate transition-all hover:border-[#B32032]/50"
              >
                <div className="text-[13.5px] font-semibold text-[#E5EAE6] flex items-center gap-2">
                  <span className="font-mono text-[#B32032] font-semibold">{i + 1}.</span>
                  {pt.title}
                </div>
                <div className="mt-1 text-[12.5px] text-[#8A968E] leading-relaxed">
                  {pt.desc}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Footer Navigation */}
        <div className="flex items-center justify-between border-t border-[#202623] pt-4">
          {/* Skip Button */}
          <button
            onClick={closeTour}
            className="text-[12.5px] font-medium text-[#8A968E] hover:text-[#E5EAE6] transition-colors px-2 py-1"
          >
            Skip Tour
          </button>

          {/* Dots Indicator */}
          <div className="flex items-center gap-1.5">
            {SLIDES.map((_, i) => (
              <button
                key={i}
                onClick={() => setCurrentSlide(i)}
                className={`h-2 rounded-full transition-all ${
                  i === currentSlide
                    ? "w-6 bg-[#B32032]"
                    : "w-2 bg-[#202623] hover:bg-[#8A968E]"
                }`}
                title={`Slide ${i + 1}`}
              />
            ))}
          </div>

          {/* Arrow Buttons */}
          <div className="flex items-center gap-2">
            <button
              onClick={prevSlide}
              disabled={currentSlide === 0}
              className="flex h-8 w-8 items-center justify-center rounded-lg border border-[#202623] bg-[#1E2321] text-[#8A968E] hover:text-[#E5EAE6] disabled:opacity-30 disabled:pointer-events-none transition-all"
              title="Previous slide"
            >
              <ChevronLeft size={16} />
            </button>
            <button
              onClick={nextSlide}
              className="flex h-8 items-center gap-1.5 rounded-lg bg-[#B32032] px-3.5 text-[13px] font-bold text-white hover:bg-[#D32F2F] transition-all shadow-md shadow-[#B32032]/20"
              title="Next slide"
            >
              <span>{currentSlide === SLIDES.length - 1 ? "Get Started" : "Next"}</span>
              <ChevronRight size={15} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
