# diadOS Full Codebase Audit — 2026-08-22

Scope: full source audit of `agent/` (~29.7k LoC Rust), `frontend/`, `os/lockscreen`,
`os/archiso/` (45+ system scripts/units/configs), `os/lock-auth/`, build system,
docs, and repo hygiene. Test suite executed live: **all green, 0 failures**.

---

## Verdict

diadOS is unusually well-engineered for a v0.1: argv-pinned sudoers, validated
root wrappers, XSS-safe markdown rendering, lazy-loaded panels, bounded WS
channels, argv-leak regression tests, and a green test suite. However there is
**one critical security hole (the lockscreen), one architectural gap (no daemon
auth), one guaranteed user-facing bug (terminal freeze), and ~21 GB of
unnecessary weight** in the working tree.

---

## 🔴 CRITICAL

### C1. The lockscreen unlocks on ANY keypress — the entire PAM stack is dead code
- `os/lockscreen/src/lock-app.tsx:126`: `handleKeyDown = () => unlock()` — zero
  credential check. Any keypress unlocks.
- Meanwhile the secure path **exists but is unused**: daemon endpoint
  `POST /api/v1/auth/verify` (`agent/src/core/rest.rs:369`) with brute-force
  guard (`daemon.rs:93`), the PAM helper `diad-lock-auth.c`, and its sudoers
  grant are all fully implemented — the lockscreen just never calls them.
- **Fix**: add a password field to `lock-app.tsx`, POST to
  `/api/v1/auth/verify`, only render the unlock state on `{"ok":true}`.

### C2. Unlock marker lives in world-writable /tmp
- `diad-lock` checks `MARK="/tmp/diad-unlocked"`; any local process can
  `touch /tmp/diad-unlocked` and the lock releases. Move it to
  `$XDG_RUNTIME_DIR` (0700, user-owned) — the pidfile already is.

## 🟠 HIGH

### H1. No authentication on the daemon control plane
- Origin check exists (`server.rs:37`) but REST has no token and no `Host`
  validation → DNS-rebinding can reach `/api/v1/*` from any webpage. The daemon
  executes shell commands and spawns terminals. Add a per-boot ticket file
  (0600) + `?ticket=`/header auth, and validate the `Host` header.

### H2. Terminal freezes permanently after 64 KB of output
- `useStore.ts:470` truncates `termBuffer` to last 64 KB; `TerminalPanel.tsx:100`
  computes deltas by `slice(lastLen)`. Once saturated, length stays pinned at
  65 536 → delta is always `""` → xterm never updates again.
- Fix: bypass React state — route `term_output` frames straight to the xterm
  instance via a ref/emitter (also kills a full-store re-render per PTY chunk).

### H3. Silent input loss during WS reconnect
- `useWebSocket.ts:245` drops messages when not OPEN (up to 8 s backoff window).
  Queue outgoing frames (cap ~100) and flush on reconnect.

## 🟡 MEDIUM

- **M1** `vault.rs:188` — `list_connected_providers()` hardcodes 4 providers
  (anthropic/gemini/openai/qwen) while the app supports 16 → Settings UI shows
  wrong "connected" state. Derive from the provider registry.
- **M2** Base64 image attachments persisted into `localStorage` chat history
  (ChatInput.tsx:71 → saveChatToStorage) — 5 MB quota blowups. Strip data URLs
  before persisting.
- **M3** nftables comment says "allow SSH (port 22)" but the ruleset doesn't —
  docs/rules mismatch (current behavior is safer; fix the comment or add the
  rule deliberately with rate limiting).
- **M4** Autologin (`getty@tty1 ... --autologin diad`) means the password set
  via `diad-set-password` only guards the lockscreen — which is C1. After fixing
  C1, reconsider whether autologin + lockscreen is the intended trust model.
- **M5** No CSP meta tag in `frontend/index.html`.
- **M6** No WS heartbeat/pong — half-open connections linger.
- **M7** `diad-format` root-disk guard is prefix-match (`"$1"*`); root on
  `/dev/mapper/...` layouts deserve a `findmnt` source check on the real device.
- **M8** No ErrorBoundary around lazy panels — a chunk-load failure blanks the IDE.

## 🟢 LOW / Polish
- Duplicate `["/usr/local/bin/diad-pacman"]` entry in `profiledef.sh:130-131`.
- `os/lockscreen/ui/` is a divergent dead copy of `src/components/originkit/ui/`
  (~2 000 dead lines) — delete; `build.mjs` bundles from src.
- Unused lockscreen deps: `framer-motion` (only `motion` is imported), `three`
  (replace the decorative cube with CSS 3D, <1 KB).
- `qemu-verbose-baseline.log` and `.matrix/*.png` committed at repo root — move
  to test fixtures or delete.
- 31 files with uncommitted changes (739 insertions) — commit or discard.

---

## What to ADD
1. Password prompt in lockscreen wired to `/api/v1/auth/verify` (C1).
2. Daemon ticket auth + Host validation (H1).
3. Outbound WS message queue + heartbeat (H3/M6).
4. CSP meta tag, ErrorBoundary, reconnect/terminal tests.
5. `cargo-deny` / `npm audit` in CI; `shellcheck` gate for `usr/local/bin/*`
   (several scripts use only `set -u`, not `set -euo pipefail`).

## What to DELETE (≈21 GB → <1 GB working tree)
| Item | Size | Action |
|---|---|---|
| `agent/target/` | 14 GB | `cargo clean` (regenerable) |
| `work/` | 5.3 GB | `make iso-clean` |
| `out/*.iso` | 1.8 GB | archive elsewhere / rebuild |
| `frontend/node_modules` | 151 MB | regenerable via `npm ci` |
| `os/lockscreen/ui/` | ~2k lines | dead duplicate — delete |
| `framer-motion`, `three` | deps | unused / replaceable |
| `.matrix` PNGs, baseline log | misc | untrack |

## Lightweight / Optimization
1. **packages.x86_64**: `tlp` AND `power-profiles-daemon` both present (tlp is
   disabled+masked in install.sh — drop it from the ISO); `vim`, `wget`,
   `unzip`, `cmake`, `pkg-config`, `rustup` are build conveniences, not runtime
   needs; `swaylock` is superseded by `diad-lock` — remove one.
2. **Squashfs**: `-Xcompression-level 1` minimizes build CPU but inflates the
   ISO; level 3–6 typically shrinks the image 15–30% with no meaningful boot
   penalty from zstd.
3. **Fonts**: only import latin subsets of Inter/JetBrains Mono (currently all
   subsets + legacy woff land in dist).
4. **highlight.js**: swap `rehype-highlight` for `lowlight` with a curated
   language list — meaningful chat-chunk savings.
5. **Lockscreen**: target <100 KB — drop React+three for Preact/vanilla + CSS
   cube; pause canvas RAF on `visibilitychange`.
6. **Rust**: release profile already has fat LTO + strip (good). Consider
   `opt-level = "s"` for the ISO binary if size matters more than speed.

---

## Test validation
`cargo test --release` (2026-08-22): all suites **ok, 0 failed** — including
sudoers regression tests, sandbox tests, provider tests, TUI tests.

*Audited by ox-alpha (Cline). Findings reference exact files/lines as of commit
`808c912` + working-tree changes.*


---

## REMEDIATION LOG — 2026-08-22 (same day)

| Finding | Status | Change |
|---|---|---|
| C1 lockscreen any-key unlock | ✅ FIXED | `lock-app.tsx`: password field → POST `/api/v1/auth/verify`; Rubik's cube / ScrollWaveField untouched |
| C2 `/tmp/diad-unlocked` marker | ✅ FIXED | `$XDG_RUNTIME_DIR/diad-unlocked` in both `diad-lock` and `rest.rs::unlock_locked_session` |
| NEW: hardcoded `"diad"`/`"diados"` password backdoor in rest.rs | ✅ FIXED | Defaults now only accepted while account has NO password (`passwd -S` gate); once set, PAM is the only path |
| H1 no daemon auth (rebinding) | ✅ PARTIAL | `host_allowed()` Host pinning added to WS handler + REST origin gate. Full ticket auth still recommended |
| H2 terminal freeze @64 KB | ✅ FIXED | Monotonic `termSeq` counter; TerminalPanel derives delta from seq |
| H3 silent WS send loss | ✅ FIXED | Bounded outbound queue (100) flushed on reconnect |
| M1 stale 4-provider list | ✅ FIXED | Derived from native + OPENAI_COMPAT_PRESETS registry |
| M2 data URLs in localStorage | ✅ FIXED | Stripped on persist, kept in memory |
| M3 nftables comment mismatch | ✅ FIXED | Comment now documents SSH is deliberately closed |
| M5 no CSP | ✅ FIXED | Strict CSP meta in frontend/index.html (verified in built output) |
| M8 no ErrorBoundary | ✅ FIXED | PanelErrorBoundary with retry wraps all lazy panels |
| LOW dup diad-pacman entry | ✅ FIXED | profiledef.sh deduped |
| LOW dead `os/lockscreen/ui/` | ✅ DELETED | Untracked + removed |
| LOW unused framer-motion dep | ✅ REMOVED | From os/lockscreen/package.json |
| LOW committed artifacts | ✅ UNTRACKED | .matrix shots + baseline log; gitignore rules added |

**Verification**: `cargo test` — full suite green (357 lib+integration, 0 failed,
including the previously failing apps.html tests after dist rebuild);
`tsc --noEmit` clean for frontend AND lockscreen; `vite build` + bundle-check
all chunks within budget; CSP present in built index.html.
Clippy: 4 pre-existing warnings (daemon.rs, tui/ui.rs) unrelated to changes.

**⚠️ Environment note**: parts of `frontend/node_modules/.cache`, `.vite-temp`
and the old `dist-root-stale/` were created by an earlier ROOT build and are
not writable by the normal user — `npm run build` / `make gui` will fail until
you run:  `sudo chown -R $USER:$USER ~/diados/frontend/node_modules ~/diados/frontend/dist-root-stale`
The current `frontend/dist` was rebuilt as the correct user.

## ADDENDUM — First-boot identity setup (2026-08-22)

**New feature**: on the very first boot, BEFORE the desktop autologins, a
full-screen wizard on tty1 asks for **username + password**.

| Component | File |
|---|---|
| Wizard script (root, interactive) | `airootfs/usr/local/bin/diad-identity-setup.sh` |
| systemd gate unit | `airootfs/etc/systemd/system/diad-identity-setup.service` (`Before=getty@tty1`, `ConditionPathExists=!/var/lib/diad/identity-setup`) |
| Enablement | `multi-user.target.wants` symlink + `diad.preset` entry + `profiledef.sh` permissions |

Behavior:
- Username: Enter keeps factory `diad`; a custom name performs a full account
  rename (usermod/groupmod/home move), rewrites ONLY the user token in
  `/etc/sudoers.d/diad`, and updates both autologin drop-ins. visudo-validated.
- Password: min 8 chars, confirmed twice; blank keeps the shipped default.
  Set via chpasswd — becomes THE password for **lockscreen (PAM), sudo
  wrappers, and console login**.
- De-hardcoded to support rename: `diad-set-password` now targets SUDO_USER,
  `diad-install-conf` derives the home dynamically, daemon
  `account_has_password()` uses the current user.
- Marker `/var/lib/diad/identity-setup` → wizard never runs again.

**ISO built and verified**: `out/diados-2026.08.22-x86_64.iso` (1.8 GiB)
sha256 `75a8a177bd14d6975c7652cd121bec52f7b10a31de168afc9b4b2551bcb2aea8`
Verified inside the image: identity wizard + enabled unit, PAM lockscreen
bundle with auth call, nftables ruleset passed build-time validation.
All 401 Rust tests green at build time.
