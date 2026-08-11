# DIAD — UI Reference (RBRO-004)

Reference for the DIAD desktop surfaces shipped in the RBRO-004 release:
the agent GUI, the Apps icon page, the Launchpad, and the Waybar glass pills.

## 1. Workspace layout

| Workspace | Surface                      | Entry            | Boot policy                         |
|-----------|------------------------------|------------------|-------------------------------------|
| 1         | DIAD AGENT (GUI or TUI)      | `http://127.0.0.1:7860` | exactly one, spawn-once (WP-03)     |
| 2         | DIAD Apps icon page          | `/apps`          | exactly one, spawn-once (WP-04)     |
| 3         | DIAD Launchpad               | `/launch`        | exactly one, spawn-once (WP-05)     |

All surfaces are chromeless Brave app-windows (`--app=...`).

## 2. DIAD AGENT (workspace 1)

- React 18 + Vite SPA served by the axum daemon (`RustEmbed` of
  `frontend/dist`); main bundle `assets/index-BV-xNFif.js`.
- WebSocket at `/ws` for the chat; HTTP API under `/api/v1`.
- **Top bar**: title, status chip, and a **⏻ power menu** (WP-06) with
  Sleep (☾), Restart (⟳), Shutdown (⏻). The menu closes on outside click or
  Escape. Each action sends the matching slash command and is always
  confirmed by the daemon — the power verbs (`systemctl suspend|reboot|
  poweroff`) are pinned in sudoers.
- Slash commands (also in the TUI): `/help`, `/tools`, `/version`,
  `/shutdown`, `/reboot`, `/sleep` — the last three always ask for
  confirmation first.
- GUI-only surfaces read `GET /api/v1/gui/fallback` on next start: if a
  previous boot fell back to TUI, the GUI shows one warning toast with the
  persisted reason, then deletes the file.

## 3. DIAD Apps icon page (workspace 2)

- Tile grid of installed apps; clicking a tile calls `POST /api/v1/launch`
  which opens the real app window. The page itself never auto-opens browsers —
  this is what killed the old WS2 flood.

## 4. DIAD Launchpad (workspace 3)

- Telegram and Slack cards; clicking calls `/api/v1/launch` for each chat.
- Auto-closes (page `window.close()`) once **both** chats report open; the
  supervisor sway-kill backstop enforces it if the browser blocks `close()`
  (checked via `/api/v1/launch/status`).
- `Super+Shift+L` relaunches the Launchpad.

## 5. Waybar (glass pills, WP-06)

- **Config**: `/etc/waybar/config` (used by the supervisor), plus
  `config.compact` and `config.full` (switched by the agent's
  `set_waybar_mode` tool).
- **Style**: `/etc/waybar/style.css` — translucent glass pills
  (`rgba` surface + backdrop blur), rounded, subtle borders.
- **Modules**: workspaces, clock, and the ⏻ power pill (glyph `\uf011`).
  Font Awesome glyphs come from `otf-font-awesome` (added to
  `packages.x86_64` in WP-06).
- Owned by `diad-supervisor.sh` — sway config never launches waybar; the
  supervisor relaunches it if it dies (10 s watchdog).

## 6. Notifications & scratchpad

- **mako** is the notification daemon (supervisor watchdog).
- **Scratchpad**: `Super+M` minimizes, `Super+Shift+M` restores. The WP-08
  limits watcher moves excess windows here (never kills them) and shows a
  rate-limited toast: *"DIAD: workspace N capped at <cap> ..."*.

## 7. Fonts & glyphs

- `otf-font-awesome` provides the pill glyphs: workspaces `\uf268`, refresh
  `\uf021`, power `\uf011`, memory `\uf538`, cpu `\uf4cb`.
- All three waybar configs are validated with `jq empty` at build time.
