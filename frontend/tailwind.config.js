/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        obsidian: "rgb(var(--bg-obsidian-rgb) / <alpha-value>)",
        surface: "rgb(var(--bg-surface-rgb) / <alpha-value>)",
        "surface-2": "rgb(var(--bg-surface-2-rgb) / <alpha-value>)",
        "surface-3": "rgb(var(--bg-surface-3-rgb) / <alpha-value>)",
        copper: "rgb(var(--accent-rgb) / <alpha-value>)",
        "copper-hover": "rgb(var(--accent-hover-rgb) / <alpha-value>)",
        "copper-dim": "rgb(var(--accent-dim-rgb) / <alpha-value>)",
        ink: "rgb(var(--text-ink-rgb) / <alpha-value>)",
        dim: "rgb(var(--text-dim-rgb) / <alpha-value>)",
        faint: "rgb(var(--text-faint-rgb) / <alpha-value>)",
        success: "rgb(var(--success-rgb) / <alpha-value>)",
        warn: "rgb(var(--warn-rgb) / <alpha-value>)",
        danger: "rgb(var(--danger-rgb) / <alpha-value>)",
        "border-subtle": "var(--border-subtle, rgba(255,255,255,0.08))",
        "border-strong": "var(--border-strong, rgba(255,255,255,0.16))",
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "monospace"],
      },
      borderRadius: {
        panel: "var(--radius-panel, 12px)",
        control: "var(--radius-control, 8px)",
      },
      backdropBlur: {
        glass: "20px",
      },
      transitionTimingFunction: {
        out: "cubic-bezier(0.22, 1, 0.36, 1)",
      },
      keyframes: {
        "fade-in": {
          "0%": { opacity: "0", transform: "translateY(4px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        "pulse-dot": {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.35" },
        },
      },
      animation: {
        "fade-in": "fade-in 0.18s ease-out",
        "pulse-dot": "pulse-dot 1.6s ease-in-out infinite",
      },
    },
  },
  plugins: [],
};
