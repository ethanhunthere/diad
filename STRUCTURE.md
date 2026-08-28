# DIAD OS — How It's Organized

> A guided tour of this project for people who **don't write code**.
> You don't need to understand programming to know where everything lives and
> what it does. This page is a map — keep it open while you browse.

---

## 1. The big picture: DIAD is three things working together

DIAD is not one program. It's **three layers** that come together to form a
complete operating system:

```
┌─────────────────────────────────────────────────────────────┐
│  THE FACE  ·  frontend/                                     │
│  The screen you look at and type into.                      │
│  A web page (React) that runs inside the browser.           │
└──────────────────────────────┬──────────────────────────────┘
                               │  talks to…
┌──────────────────────────────▼──────────────────────────────┐
│  THE BRAIN ·  agent/                                        │
│  The background program that thinks and does the work.      │
│  Written in Rust. Talks to AI providers and runs tools.     │
└──────────────────────────────┬──────────────────────────────┘
                               │  is installed inside…
┌──────────────────────────────▼──────────────────────────────┐
│  THE BODY  ·  os/                                           │
│  The actual operating system you boot and install.          │
│  Arch Linux + Sway + all of DIAD's files and settings.      │
└─────────────────────────────────────────────────────────────┘
```

A useful way to remember it:

- **`frontend/`** is the **storefront** — what the customer sees.
- **`agent/`** is the **back office** — where the real work happens.
- **`os/`** is the **building itself** — the walls, wiring and plumbing that
  everything lives in.

The other folders are the **manual**, the **test lab**, and the **brand kit**
(explained below).

---

## 2. The top-level folders (the short version)

Open the project and you'll see these folders. Here is what each one is, in
plain words:

| Folder | What it is | Think of it as… |
|--------|------------|-----------------|
| `agent/` | The **brain** — a Rust program that runs in the background, talks to AI, and performs actions (files, hardware, network, system). | The back office |
| `frontend/` | The **face** — the React web app you see and click on. | The storefront |
| `os/` | The **body** — everything that becomes the bootable operating system (the `.iso`). | The building |
| `scripts/` | The **test lab** — tools to run DIAD in a virtual machine, check it, and audit it. | The inspection crew |
| `docs/` | The **owner's manual** — written explanations (this file's family). | The manual |
| `assets/` | The **brand kit** — the DIAD logos in a few sizes and colors. | The letterhead |
| `Makefile` | The **assembly line** — one command (`make iso`) builds the whole OS. | The factory button |
| `install.sh` | The **installer** — puts DIAD onto an existing Arch Linux machine. | The handyman |
| `README.md` | The **front door** — the first page you read. | The sign on the door |
| `STRUCTURE.md` | This file — the map. | The fire-escape map |
| `.gitignore` | A list of junk files that should never be saved in version control. | The "don't keep this" list |

> **What about `out/` and `work/`?** Those two folders only appear **after** you
> build the ISO. `out/` is the finished product (the `.iso` file), and `work/`
> is the messy temporary workshop `mkarchiso` uses while building. They are
> build outputs, not source code — you can safely delete them.

---

## 3. Inside `os/` — the operating system itself (the deep dive)

This is the folder most people want to understand, because **this is the OS**.

```
os/
├── archiso/              ← the "recipe" that bakes DIAD into an installable .iso
│   ├── profiledef.sh     ← the master recipe (what to include, what permissions)
│   ├── packages.x86_64   ← the shopping list of programs to install
│   ├── pacman.conf       ← which software repositories to pull from
│   ├── grub/             ← the boot menu you see when the computer starts
│   └── airootfs/         ← the "future filesystem" — becomes the OS's files
├── lock-auth/            ← a tiny C program that checks your password (PAM)
└── lockscreen/           ← the lock-screen web page (built into airootfs)
```

### 3.1 `os/archiso/airootfs/` — the OS's actual files

`mkarchiso` takes everything in `airootfs/` and copies it into the operating
system you boot. Think of `airootfs/` as **a mirror of the OS's own drive**:

- A file at `airootfs/etc/sway/config` will exist on the installed OS as
  `/etc/sway/config`.
- A file at `airootfs/usr/local/bin/diad` becomes `/usr/local/bin/diad`.

So when you browse `airootfs/`, you are literally browsing the OS before it's
baked.

#### The important neighborhoods inside `airootfs/`:

| Path | What lives there |
|------|------------------|
| `etc/sway/` | **The window manager settings.** How windows look, keybindings, workspaces, the lock-screen trigger. This is the "desktop behavior" file. |
| `etc/waybar/` | **The top bar** (clock, wifi, battery, workspace pills) and its styling. |
| `etc/wofi/` | **The app launcher** styling (the "start menu" that pops up). |
| `etc/mako/` | **Notifications** — how pop-ups look. |
| `etc/swaylock/` | The *native* lock-screen config (DIAD also has a fancier web lock screen — see `os/lockscreen`). |
| `etc/systemd/system/` | **The startup services.** What turns on automatically when the computer boots (firewall, identity setup, performance tuning). |
| `etc/sudoers.d/` | **Who is allowed to run what as administrator.** DIAD pins down exactly which commands may run with elevated rights. |
| `etc/nftables.conf` | **The firewall rules.** |
| `etc/hostname` | The machine's name. |
| `etc/locale.conf` | Language / timezone settings. |
| `etc/environment` | Global settings applied to every program. |
| `etc/skel/` | **The template** for a new user's home folder (copied when a user is created). |
| `usr/local/bin/` | **DIAD's own little programs** — the `diad-*` scripts (see below). |
| `usr/share/diad/lockscreen/dist/` | The **lock-screen web page** (built from `os/lockscreen`). |
| `usr/share/backgrounds/` | Wallpapers. |
| `home/diad/` | **The default user's home folder** — their personal settings (shell, terminal, notifications). |
| `var/lib/diad/` | Runtime data DIAD writes while it runs. |

#### The `diad-*` programs in `usr/local/bin/` (the OS's "staff")

Every file starting with `diad-` is a small program that does one job. A few
you'll recognize from using the OS:

| Program | What it does |
|---------|--------------|
| `diad-supervisor.sh` | **The conductor.** Runs at login, starts the daemon, opens the GUI, arranges the three workspaces. |
| `diad-identity-setup.sh` | The **first-boot wizard** — asks for your username & password and creates your account. |
| `diad-login` | The login prompt on the first text screen. |
| `diad-lock` | Locks the screen (shows the lock screen). |
| `diad-lock-auth` | The compiled helper that actually **checks your password** against the system. |
| `diad-wifi.sh` | The Wi-Fi picker. |
| `diad-power.sh` | The power menu (shutdown / reboot / sleep). |
| `diad-context-menu.sh` | The right-click menu. |
| `diad-clipboard.sh` | Clipboard management. |
| `diad-screenshot` | Takes a screenshot. |
| `diad-perf-tier.sh` | Detects how powerful the machine is and tunes DIAD to match. |
| `diad-hardware-setup.sh` | Detects hardware on first boot. |
| `diad-network-setup.sh` | Network setup. |
| `diad-storage-setup.sh` | Storage / disk setup. |
| `diad-selfupdate` | Updates DIAD itself. |

> The big one — `diad-agent` — is not a shell script. It's the **compiled
> Rust brain** (from `agent/`), copied here so the OS ships with it built-in.

### 3.2 `os/archiso/grub/` — the boot menu

- `grub.cfg` — the menu you see right after power-on ("Boot DIAD" etc.), plus
  the quiet/fast boot settings.
- `themes/grub-theme-diad/` — how that menu *looks* (background image, colors).

### 3.3 `os/lockscreen/` — the lock screen (a small web app)

The lock screen is itself a tiny web page (React), built into the OS. It lives
in source form here (`src/lock-app.tsx`) and gets compiled into
`archiso/airootfs/usr/share/diad/lockscreen/dist/` during the build.

### 3.4 `os/lock-auth/` — the password checker

A single C file (`diad-lock-auth.c`) that verifies your password using the
system's own authentication (PAM). This is the piece that decides whether the
lock screen lets you back in.

---

## 4. Inside `agent/` — the brain

The agent is a **Rust** program. Its source is organized by *responsibility*:

| Path | Responsibility |
|------|----------------|
| `agent/src/main.rs` | The entry point — what runs when the program starts. |
| `agent/src/lib.rs` | The library root — shared pieces. |
| `agent/src/core/` | The **heart**: the daemon, the local web API, the idle watcher, performance, notes, the terminal bridge. |
| `agent/src/orchestrator/` | The **dispatcher** — decides what to do with your request and manages conversations. |
| `agent/src/providers/` | The **translators** — one file per AI company (Anthropic, Google, OpenAI, Qwen, …) so DIAD can talk to all of them. |
| `agent/src/tools/` | The **toolbox** — the safe, sandboxed actions DIAD can perform (files, hardware, network, system). |
| `agent/src/auth/` | Login / OAuth / the secure key vault. |
| `agent/src/tui/` | The **keyboard-only** interface (`diad-agent --tui`). |
| `agent/src/config/` | Settings. |
| `agent/tests/` | Automated checks that prove the brain works. |

---

## 5. Inside `frontend/` — the face

The GUI is a **React + TypeScript** web app:

| Path | Responsibility |
|------|----------------|
| `frontend/src/panels/` | The big areas of the screen (chat, editor, terminal, agents). |
| `frontend/src/components/` | Reusable building blocks (buttons, cards, diff views). |
| `frontend/src/services/` | Code that talks to the agent's API. |
| `frontend/src/stores/` | The "memory" of the UI state (what's open, what's selected). |
| `frontend/src/hooks/` | Small reusable behavior helpers. |
| `frontend/src/lib/` | Utilities. |
| `frontend/src/styles/` | Styling. |
| `frontend/src/assets/` | Images used inside the GUI. |
| `frontend/public/` | Static files served as-is. |

> When you run `make iso`, the frontend is compiled into `frontend/dist/` and
> **baked directly into the Rust brain** (`rust-embed`), so the shipped OS has
> the GUI built-in with no separate web server needed.

---

## 6. The three workspaces (how the desktop is arranged)

When you log in, `diad-supervisor.sh` arranges the screen into three named
workspaces (press the number keys 1/2/3, or `$mod`+1/2/3 to jump between them):

| Workspace | Name | What's there |
|-----------|------|--------------|
| **1** | DIAD AGENT | The AI interface (the GUI). |
| **2** | BROWSER | The web browser. |
| **3** | COMMS | Telegram + Slack (your communication apps). |

This layout is defined in `os/archiso/airootfs/etc/sway/config` and staged by
`diad-supervisor.sh`.

---

## 7. How a change flows (from idea to installed OS)

This is the "factory line" — useful to understand how your edits become a
bootable system:

```
1. Edit source
   ├─ Change the GUI  → edit frontend/…
   ├─ Change the brain → edit agent/…
   └─ Change the OS    → edit os/archiso/…  (or os/lockscreen, os/lock-auth)

2. Build
   make iso      (runs: build GUI → build brain → bake the .iso)
                 → produces out/diados-YYYY.MM.DD-x86_64.iso

3. Flash
   sudo dd if=out/diados-….iso of=/dev/sda …
                 → writes the OS onto a USB drive

4. Boot
   → GRUB menu → login → Sway desktop (the three workspaces)
```

---

## 8. Cheat sheet — "I want to change X, where do I look?"

| I want to… | Look here |
|-----------|-----------|
| Change the desktop layout / workspaces / keybindings | `os/archiso/airootfs/etc/sway/config` |
| Change the top bar (clock, wifi, battery) | `os/archiso/airootfs/etc/waybar/` |
| Change how notifications look | `os/archiso/airootfs/etc/mako/config` |
| Change the app-launcher look | `os/archiso/airootfs/etc/wofi/` |
| Change the lock screen | `os/lockscreen/src/lock-app.tsx` |
| Change how passwords are checked | `os/lock-auth/diad-lock-auth.c` |
| Change the first-boot account wizard | `os/archiso/airootfs/usr/local/bin/diad-identity-setup.sh` |
| Change the boot menu | `os/archiso/grub/grub.cfg` |
| Add/remove installed programs | `os/archiso/packages.x86_64` |
| Change the GUI (the AI screen) | `frontend/src/` |
| Change what the AI can do (tools) | `agent/src/tools/` |
| Add a new AI provider | `agent/src/providers/` |
| Change the DAEMON's local web API | `agent/src/core/rest.rs` |
| Read the docs | `docs/` (see `docs/README.md` for the index) |

---

## 9. Good to know (housekeeping)

- **`out/` and `work/`** are build outputs — regenerated by `make iso`, safe to
  delete.
- **`node_modules/`, `target/`, `dist/`** are downloaded/built dependencies —
  never edited by hand, regenerated automatically.
- **`*.root-stale/` folders** (e.g. `frontend/node_modules.root-stale`) are old
  leftovers from a previous run as the root user; they're ignored and harmless.
- **`docs/`** already contains a full library (architecture, boot, security,
  testing, UI, troubleshooting). Start with `docs/README.md` for the index.
