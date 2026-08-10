# DIAD — Troubleshooting

Exact commands for the three classic failure classes (spec 12.3).

## 1. Boot fails or the GUI never appears

```bash
# What happened on this boot?
journalctl -b -p warning --no-pager | head -40

# Boot chain checkpoints (each must exist):
systemctl status diad-perf-tier.service diad-firewall.service
cat ~/.diad/logs/boot.log          # supervisor narrative
cat ~/.diad/logs/*.log             # per-script logs

# Is the daemon up?
curl -s http://127.0.0.1:7860/health     # expect: ok
curl -s http://127.0.0.1:7860/api/v1/status | jq

# Is sway running the supervisor at all?
swaymsg -t get_tree | jq '.. | select(.name? == "DIAD")' 2>/dev/null
```

Common causes:

| Symptom                    | Fix                                                        |
|----------------------------|------------------------------------------------------------|
| Black screen after GRUB    | Boot the **verbose** GRUB entry (loglevel=7) and read the failing unit |
| `health` connection refused| `diad-agent --daemon` manually in a foot; read stderr       |
| GUI window blank           | `brave --app=http://127.0.0.1:7860` manually; check console |
| First-run never shows      | `rm ~/.diad/.first-run-done` and reboot (marker gate)       |

## 2. Wrong perf tier

```bash
cat /etc/diad/perf-tier                 # current tier word
cat /etc/diad/hw-profile.json | jq      # every detected input
sudo diad-perf-tier.sh                  # re-detect now (writes both files)
```

The classifier is `core/perf.rs::classify` (unit-tested). If a detection
input failed, `hw-profile.json` shows the fallback (0 RAM, `software` GPU
…). Fix the underlying detection (missing `pciutils`? no `/sys` entries in
a container?) — do not hardcode tiers in scripts.

## 3. High idle CPU/RSS with the agent closed

```bash
# Is the daemon actually idle?
curl -s http://127.0.0.1:7860/api/v1/status | jq .idle_stage
diad-bench.sh --watch                   # live RSS/CPU picture

# Which periodic tasks are enabled? They must all be off at IDLE1+:
journalctl -u diad-agent --no-pager | grep 'diad daemon'
```

Expected log sequence after closing the GUI: `diad daemon idle (stage1)`
after ~30 s, `(stage2)` after ~5 min. If stages never engage, a client is
still connected (`clients_count` > 0) — find it:

```bash
ss -tnp '( sport = :7860 )'
```

If RSS stays high in IDLE2, capture the regression evidence and run:

```bash
diad-bench.sh --check    # fails >15% vs /etc/diad/perf-baseline.json
```

## Reset procedures

```bash
# Full desktop reset (keeps your vault keys):
rm -f ~/.diad/.first-run-done ~/.diad/logs/*.log && reboot

# Fresh perf baseline after intentional tuning:
sudo rm /etc/diad/perf-baseline.json && diad-bench.sh --check

# Re-run all setup scripts without rebooting:
sudo diad-perf-tier.sh && diad-network-setup.sh && diad-power-setup.sh
```
