# `os/` — The Body (the operating system itself)

Everything in this folder becomes the **actual operating system** you boot and
install — the `.iso` file.

| Path | What it is |
|------|------------|
| `archiso/` | The "recipe" that bakes DIAD into an installable ISO (see its own README) |
| `lockscreen/` | The lock-screen web app (built into `archiso/`) |
| `lock-auth/` | A tiny C program that verifies your password via PAM |

Build with `make iso` (from the repo root). See [`../STRUCTURE.md`](../STRUCTURE.md)
for the full picture.
