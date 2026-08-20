import { useEffect, useRef, useState } from "react";
import { Send, CornerDownLeft, ImagePlus, X } from "lucide-react";
import { useStore } from "../stores/useStore";

const SLASH_COMMANDS: { cmd: string; desc: string }[] = [
  { cmd: "/wifi", desc: "Scan & connect Wi-Fi" },
  { cmd: "/status", desc: "Hardware diagnostics" },
  { cmd: "/model", desc: "Select AI model" },
  { cmd: "/tools", desc: "List OS & coding tools" },
  { cmd: "/theme", desc: "Change appearance & colors" },
  { cmd: "/help", desc: "Show command reference" },
];

interface AttachedImage {
  id: string;
  dataUrl: string;
  name: string;
  size: number;
}

/**
 * World-class auto-grow chat input with multimodal image attachments,
 * clipboard paste support, drag & drop, and industrial basalt styling.
 */
export function ChatInput({
  onSend,
  disabled,
}: {
  onSend: (text: string, images?: string[]) => void;
  disabled?: boolean;
}) {
  const input = useStore((s) => s.input);
  const setInput = useStore((s) => s.setInput);
  const streaming = useStore((s) => s.streaming);
  const ref = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [images, setImages] = useState<AttachedImage[]>([]);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuIdx, setMenuIdx] = useState(0);
  const [isDragging, setIsDragging] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 220)}px`;
  }, [input]);

  useEffect(() => {
    ref.current?.focus();
  }, [streaming]);

  const addFiles = (files: FileList | File[]) => {
    Array.from(files).forEach((file) => {
      if (!file.type.startsWith("image/")) return;
      const reader = new FileReader();
      reader.onload = (e) => {
        const dataUrl = e.target?.result as string;
        if (dataUrl) {
          setImages((prev) => [
            ...prev,
            {
              id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
              dataUrl,
              name: file.name,
              size: file.size,
            },
          ]);
        }
      };
      reader.readAsDataURL(file);
    });
  };

  const removeImage = (id: string) => {
    setImages((prev) => prev.filter((img) => img.id !== id));
  };

  const submit = () => {
    const text = input.trim();
    if (!text && images.length === 0) return;
    const imageUrls = images.map((img) => img.dataUrl);
    onSend(text || "Uploaded image", imageUrls.length > 0 ? imageUrls : undefined);
    setInput("");
    setImages([]);
    setMenuOpen(false);
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    const imageFiles: File[] = [];
    for (let i = 0; i < items.length; i++) {
      if (items[i].type.startsWith("image/")) {
        const file = items[i].getAsFile();
        if (file) imageFiles.push(file);
      }
    }
    if (imageFiles.length > 0) {
      addFiles(imageFiles);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      addFiles(e.dataTransfer.files);
    }
  };

  const handleKey = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "/" && !e.shiftKey && !e.metaKey && !e.ctrlKey && input === "") {
      setMenuOpen(true);
      setMenuIdx(0);
      return;
    }
    if (menuOpen) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMenuIdx((i) => (i + 1) % SLASH_COMMANDS.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setMenuIdx((i) => (i - 1 + SLASH_COMMANDS.length) % SLASH_COMMANDS.length);
        return;
      }
      if (e.key === "Tab" || e.key === "Enter") {
        e.preventDefault();
        const cmd = SLASH_COMMANDS[menuIdx];
        setInput(cmd.cmd);
        setMenuOpen(false);
        return;
      }
      if (e.key === "Escape") {
        setMenuOpen(false);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
      return;
    }
    if (e.key === "Escape" && input === "") {
      setMenuOpen(false);
      return;
    }
  };

  const pick = (cmd: string) => {
    setInput(cmd);
    setMenuOpen(false);
    ref.current?.focus();
  };

  return (
    <div
      className={`relative border-t border-[#202623] bg-[#0F1110] px-3.5 sm:px-6 py-3 sm:py-4 transition-colors ${
        isDragging ? "bg-[#1E2321] border-[#B32032]" : ""
      }`}
      onDragOver={(e) => {
        e.preventDefault();
        setIsDragging(true);
      }}
      onDragLeave={() => setIsDragging(false)}
      onDrop={handleDrop}
    >
      <input
        type="file"
        ref={fileInputRef}
        accept="image/*"
        multiple
        className="hidden"
        onChange={(e) => {
          if (e.target.files) addFiles(e.target.files);
          e.target.value = "";
        }}
      />

      {/* "/" command menu */}
      {menuOpen && (
        <div className="glass absolute bottom-full left-3.5 sm:left-6 mb-2.5 w-76 sm:w-84 overflow-hidden rounded-xl border border-[#202623] bg-[#161918] shadow-2xl animate-fade-in">
          <div className="border-b border-[#202623] bg-[#1E2321] px-4 py-2.5 text-[11.5px] font-bold uppercase tracking-wider text-[#B32032] font-mono">
            ✦ Quick Actions
          </div>
          <div className="p-1.5">
            {SLASH_COMMANDS.map((c, i) => (
              <button
                key={c.cmd}
                className={`flex w-full items-center gap-3 rounded-lg px-3.5 py-2 text-left text-[13.5px] transition-colors ${
                  i === menuIdx ? "bg-[#B32032]/20 text-[#E5EAE6] font-medium" : "text-[#8A968E] hover:text-[#E5EAE6] hover:bg-[#1E2321]"
                }`}
                onMouseEnter={() => setMenuIdx(i)}
                onClick={() => pick(c.cmd)}
              >
                <span className="font-mono text-[#B32032] font-semibold">{c.cmd}</span>
                <span className="flex-1 text-right text-[12px] text-[#8A968E]">{c.desc}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Attached Images Preview Row */}
      {images.length > 0 && (
        <div className="mb-3 flex flex-wrap gap-2.5 animate-fade-in">
          {images.map((img) => (
            <div
              key={img.id}
              className="group relative flex items-center gap-2 rounded-xl border border-[#202623] bg-[#161918] p-1.5 pr-2.5 shadow-md hover:border-[#B32032]/60 transition-all"
            >
              <img
                src={img.dataUrl}
                alt={img.name}
                className="h-12 w-12 rounded-lg object-cover border border-[#202623]"
              />
              <div className="flex flex-col">
                <span className="max-w-[120px] truncate text-[12px] font-medium text-[#E5EAE6]">
                  {img.name}
                </span>
                <span className="text-[10.5px] text-[#8A968E]">
                  {(img.size / 1024).toFixed(0)} KB
                </span>
              </div>
              <button
                onClick={() => removeImage(img.id)}
                className="ml-1 flex h-5 w-5 items-center justify-center rounded-full bg-[#1E2321] text-[#8A968E] hover:bg-[#B32032] hover:text-white transition-colors"
                title="Remove image"
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="flex items-end gap-2.5 sm:gap-3">
        {/* Upload Image Button */}
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          disabled={disabled}
          className="flex h-[48px] w-[48px] shrink-0 items-center justify-center rounded-xl border border-[#202623] bg-[#161918] text-[#8A968E] hover:border-[#B32032] hover:text-[#E5EAE6] hover:bg-[#1E2321] disabled:opacity-40 transition-all shadow-sm"
          title="Upload images for multimodal analysis"
          aria-label="Upload images"
        >
          <ImagePlus size={20} />
        </button>

        {/* Chat Textarea */}
        <textarea
          ref={ref}
          value={input}
          rows={1}
          disabled={disabled}
          placeholder={
            disabled
              ? "Connecting to local agent daemon…"
              : streaming
                ? "DIAD Agent is reasoning and executing tools…"
                : 'Ask anything, execute OS intents, paste images, or type "/" for commands…'
          }
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKey}
          onPaste={handlePaste}
          className="min-h-[48px] flex-1 resize-none rounded-xl border border-[#202623] bg-[#090A0A] px-4 py-3 text-[15px] leading-relaxed text-[#E5EAE6] placeholder:text-[#525D56] focus:border-[#B32032] focus:outline-none focus:ring-2 focus:ring-[#B32032]/30 disabled:opacity-60 font-sans shadow-inner"
        />

        {/* Send Button */}
        <button
          onClick={submit}
          disabled={disabled || (!input.trim() && images.length === 0)}
          className="flex h-[48px] w-[48px] shrink-0 items-center justify-center rounded-xl bg-[#B32032] text-white font-bold hover:bg-[#D32F2F] active:scale-95 disabled:opacity-30 disabled:pointer-events-none transition-all shadow-lg shadow-[#B32032]/25"
          title="Send intent (Enter)"
          aria-label="Send message"
        >
          <Send size={19} />
        </button>
      </div>

      <div className="mt-2 flex items-center justify-between text-[11.5px] text-[#525D56] font-mono">
        <span className="flex items-center gap-1.5">
          <CornerDownLeft size={12} className="text-[#B32032]" /> Enter to send · Shift+Enter new line
        </span>
        <span className="truncate">Drag / paste images · / for tools · F5 TUI</span>
      </div>
    </div>
  );
}
