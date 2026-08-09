/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // COPPER OBSIDIAN — DIAD AGENT design system
        obsidian: "#0B0908", // app background
        surface: "#14100D", // raised surfaces
        "surface-2": "#1B1511", // hover / elevated
        "surface-3": "#221B15", // active
        copper: "#C87F4A", // primary accent
        "copper-hover": "#E0955C",
        "copper-dim": "#8A5A33",
        ink: "#EDE6DE", // primary text
        dim: "#8A7F75", // secondary text
        faint: "#5C534A", // tertiary text / icons
        success: "#6FBF8B",
        warn: "#E0B060",
        danger: "#E07A7A",
        "border-subtle": "rgba(255,255,255,0.06)",
        "border-strong": "rgba(255,255,255,0.12)",
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "monospace"],
      },
      borderRadius: {
        panel: "12px",
        control: "8px",
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
