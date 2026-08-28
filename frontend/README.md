# `frontend/` — The Face

This is the **DIAD AGENT GUI** — the web app you see and interact with
(chat, tool-call cards, AI-edit diffs, code editor, terminal, agents hub).

Built with React + TypeScript + Vite + Tailwind. When you run `make iso`, it is
compiled into `dist/` and **baked into the Rust daemon** (`rust-embed`), so the
shipped OS has the GUI built-in.

## Folder map

| Path | What it does |
|------|--------------|
| `src/panels/` | The big screen areas (chat, editor, terminal, agents) |
| `src/components/` | Reusable building blocks |
| `src/services/` | Code that talks to the agent's API |
| `src/stores/` | The UI's memory (what's open, what's selected) |
| `src/hooks/` | Reusable behavior helpers |
| `src/lib/` | Utilities |
| `src/styles/` | Styling |
| `public/` | Static files served as-is |

Build with `make gui` (from the repo root). See [`../STRUCTURE.md`](../STRUCTURE.md)
for the full picture.
