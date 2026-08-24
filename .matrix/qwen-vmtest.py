#!/usr/bin/env python3
"""
DIAD RBRO-004 — QEMU interactive matrix harness.

Drives the built ISO in QEMU headlessly:
  - ttyS0  = kernel console -> unix socket (interactive: postmount break shell)
  - ttyS1  = injected matrix-shell root shell -> unix socket (interactive)
  - monitor= unix socket (screendump)

Access path (debug-shell/serial-getty are MASKED in the ISO, so we use
mkinitcpio `break=postmount`, which fires after the root is mounted at
/new_root, and inject TWO files into the overlay root):
  1. getty@tty1 autologin drop-in  -> diad session starts on tty1 (sway, supervisor)
  2. matrix-shell.service (root shell on /dev/ttyS1, sysinit.target) -> P_READY_MARK
Then `exit` resumes switch_root -> systemd boots normally.

Usage: matrix.py <entry-id> [--mem 4096] [--full-session] [--no-poweroff]
"""
import os, re, select, socket, subprocess, sys, time, base64

BASE = "/home/f0id/diados/.matrix"
os.makedirs(BASE, exist_ok=True)

# Fixed source files overlaid into the boot (verifies fixes without rebuild).
# base64 over the tty avoids quoting issues; decoded into the overlay root
# while still at the postmount break (before systemd starts).
FIX_SOURCES = {
    "/new_root/etc/sway/config": "/home/f0id/diados/os/archiso/airootfs/etc/sway/config",
    "/new_root/usr/local/bin/diad-supervisor.sh": "/home/f0id/diados/os/archiso/airootfs/usr/local/bin/diad-supervisor.sh",
    "/new_root/etc/waybar/style.css": "/home/f0id/diados/os/archiso/airootfs/etc/waybar/style.css",
    "/new_root/home/diad/.bashrc": "/home/f0id/diados/.matrix/bashrc.diag",
}

ENTRY = sys.argv[1]
MEM = 4096
FULL_SESSION = False
NO_POWEROFF = False
DISK = None
for a in sys.argv[2:]:
    if a == "--full-session":
        FULL_SESSION = True
    elif a == "--no-poweroff":
        NO_POWEROFF = True
    elif a.startswith("--mem="):
        MEM = int(a.split("=", 1)[1])
    elif a.startswith("--disk="):
        DISK = a.split("=", 1)[1]

CON_SOCK = f"{BASE}/con-{ENTRY}.sock"
CONSOLE_LOG = f"{BASE}/console-{ENTRY}.log"
DBG_SOCK = f"{BASE}/dbg-{ENTRY}.sock"
MON_SOCK = f"{BASE}/mon-{ENTRY}.sock"
QEMU_ERR = f"{BASE}/qemu-{ENTRY}.err"
SHOT_PPM = f"{BASE}/shot-{ENTRY}.ppm"
SHOT_PNG = f"{BASE}/shot-{ENTRY}.png"

for p in (CON_SOCK, DBG_SOCK, MON_SOCK):
    if os.path.exists(p):
        os.unlink(p)

import glob
iso_candidates = sorted(glob.glob("/home/f0id/diados/out/diados-*.iso"))
ISO = iso_candidates[-1] if iso_candidates else "/home/f0id/diados/out/diados-2026.08.17-x86_64.iso"
KERNEL = "/home/f0id/diados/work/iso/diados/boot/x86_64/vmlinuz-linux-lts"
INITRD = "/home/f0id/diados/work/iso/diados/boot/x86_64/initramfs-linux-lts.img"
def get_iso_uuid(iso_path):
    try:
        out = subprocess.check_output(["blkid", "-o", "value", "-s", "UUID", iso_path], text=True).strip()
        if out:
            return out
    except Exception:
        pass
    return "2026-08-17-08-41-49-00"

UUID = get_iso_uuid(ISO)


def entry_args(entry):
    if entry in ("1-default", "q1-full", "q4-persist", "q5-keys"):
        return "quiet loglevel=3 systemd.show_status=0 fsck.mode=skip rd.timeout=10 copytoram=n"
    if entry in ("2-verbose", "q6-verbose"):
        return "loglevel=7 fsck.mode=skip rd.timeout=10 copytoram=n"
    if entry == "3-ram":
        return "quiet loglevel=3 systemd.show_status=0 fsck.mode=skip rd.timeout=10 copytoram"
    raise SystemExit(f"unknown entry {entry}")


def build_qemu(entry):
    args = entry_args(entry)
    cmd = [
        "qemu-system-x86_64", "-enable-kvm", "-cpu", "host", "-m", str(MEM), "-smp", "2",
        "-machine", "q35",
        "-kernel", KERNEL,
        "-initrd", INITRD,
        "-append", (
            "console=ttyS0,115200 archisobasedir=diados "
            f"archisosearchuuid={UUID} {args} break=postmount"
        ),
        "-drive", f"file={ISO},media=cdrom,readonly=on",
        "-netdev", "user,id=n0", "-device", "e1000,netdev=n0",
        "-vga", "virtio", "-display", "none",
        "-serial", f"unix:{CON_SOCK},server=on,wait=off",
        "-serial", f"unix:{DBG_SOCK},server=on,wait=off",
        "-monitor", f"unix:{MON_SOCK},server=on,wait=off",
        "-no-reboot",
    ]
    if DISK:
        cmd += ["-drive", f"file={DISK},if=virtio,format=qcow2"]
    return cmd


def connect(path, timeout=180):
    end = time.time() + timeout
    while time.time() < end:
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(path)
            return s
        except (FileNotFoundError, ConnectionRefusedError):
            time.sleep(0.5)
    raise TimeoutError(f"cannot connect to {path}")


class Session:
    def __init__(self, sock, logf):
        self.sock = sock
        self.logf = logf
        self.buf = b""

    def _drain(self, seconds):
        end = time.time() + seconds
        got = b""
        while time.time() < end:
            r, _, _ = select.select([self.sock], [], [], 0.25)
            if r:
                try:
                    c = self.sock.recv(65536)
                except Exception:
                    c = b""
                if not c:
                    break
                got += c
            elif got:
                break
        if got:
            self.buf += got
            self.logf.write(got.decode(errors="replace"))
            self.logf.flush()
        return got

    def send(self, text):
        self.sock.sendall(text.encode() + b"\n")

    def expect(self, pattern, timeout=120):
        rx = re.compile(pattern.encode())
        end = time.time() + timeout
        while time.time() < end:
            m = rx.search(self.buf)
            if m:
                out = self.buf[: m.end()]
                self.buf = self.buf[m.end():]
                return out
            self._drain(1.0)
        raise TimeoutError(
            f"TIMEOUT waiting {pattern!r}; tail={self.buf[-400:]!r}"
        )

    def sleep(self, seconds):
        self._drain(seconds)


VFY_SCRIPT = r"""cat > /tmp/vfy.sh <<'VFY'
#!/bin/bash
D(){ runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 SWAYSOCK="${SWAYSOCK:-}" "$@"; }
echo "== [0] wait for sway IPC + daemon (up to 300s, slow RAM-mode boots) =="
for i in $(seq 1 60); do
  SWAYSOCK=$(ls -t /run/user/1000/sway-ipc.*.sock 2>/dev/null | head -1)
  SWAYOK=no
  if [ -n "$SWAYSOCK" ] && runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 SWAYSOCK="$SWAYSOCK" swaymsg -t get_version >/dev/null 2>&1; then
    SWAYOK=yes
  fi
  HTTPOK=$(curl -s -m 3 -o /dev/null -w '%{http_code}' "http://127.0.0.1:7860/health" 2>/dev/null)
  if [ "$SWAYOK" = yes ] && [ "$HTTPOK" = 200 ]; then
    echo "  ready after ${i}x5s (sock=${SWAYSOCK} http=${HTTPOK})"
    break
  fi
  sleep 5
done
SWAYSOCK=$(ls -t /run/user/1000/sway-ipc.*.sock 2>/dev/null | head -1)
echo "SWAYSOCK=${SWAYSOCK}"
echo "  (settling 15s for supervisor app placement on WS2/WS3)"
sleep 15
echo "== [1] processes =="
ps -eo pid,comm,args | grep -E 'sway|waybar|mako|supervisor|diad-agent|foot|chromium|brave|cliphist|swaynag' | grep -vE 'grep|ps -eo' | head -40
echo "== [1b] swaynag present? =="
pgrep -a swaynag || echo "  (no swaynag)"
echo "== [2] logind sessions =="
loginctl list-sessions --no-legend
echo "== [2b] failed units =="
systemctl --failed --no-legend | head -5
echo "== [3] HTTP =="
for u in / /health /api/v1/status /apps /launch; do
  code=$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://127.0.0.1:7860$u")
  echo "  $u -> $code"
done
echo "== [4] sway tree (as diad) =="
echo "  swaynag_now: $(pgrep -a swaynag | tr '\n' ';' || echo none)"
D swaymsg -t get_tree > /tmp/tree.json 2>/tmp/tree.err
if [ -s /tmp/tree.err ]; then echo "SWAYMSG_ERR: $(head -c 300 /tmp/tree.err)"; fi
python3 - <<'PY'
import json
t = json.load(open('/tmp/tree.json'))
ws = {}
def leaves(n):
    kids = n.get('nodes', []) + n.get('floating_nodes', [])
    if not kids:
        return [n]
    out = []
    for k in kids:
        out += leaves(k)
    return out
def walk(n):
    if n.get('type') == 'workspace':
        ws[n.get('name')] = [(c.get('app_id') or c.get('name') or c.get('type')) for c in leaves(n)]
    for c in n.get('nodes', []) + n.get('floating_nodes', []):
        walk(c)
for c in t.get('nodes', []):
    walk(c)
total = 0
for name in sorted(ws):
    n = len(ws[name])
    total += n
    print(f"  WS[{name}] windows={n} :: {ws[name]}")
print(f"  TOTAL_WINDOWS={total}")
PY
echo "== [5] supervisor boot.log =="
tail -n 45 /home/diad/.diad/logs/boot.log 2>/dev/null || echo "  (no boot.log)"
echo "== [6] daemon log tail =="
tail -n 8 /home/diad/.diad/daemon.log 2>/dev/null || echo "  (no daemon log)"
echo "== [7] waybar configs =="
ls -l /etc/waybar/config /etc/waybar/style.css 2>/dev/null
echo "== [8] sway config validate (headless, FULL) =="
runuser -u diad -- env -u WAYLAND_DISPLAY WLR_BACKENDS=headless sway -C -c /etc/sway/config 2>&1 | grep -viE 'libEGL|eglInitialize|EGL_NOT|eglQueryDevice|kms_swrast|falling back|wlr.*EGL' | head -30 || true
echo "  (filtered to config errors only; empty above = clean)"
echo "== [8a] sway -d -C debug validate (shows EVERY command result) =="
runuser -u diad -- env -u WAYLAND_DISPLAY WLR_BACKENDS=headless sway -d -C -c /etc/sway/config 2>&1 | grep -iE '\[ERROR\]|\[WARN|config|fail|invalid|unable|cannot|not found|no such' | grep -viE 'libEGL|eglInitialize|EGL_NOT|eglQueryDevice|kms_swrast|falling back|wlr.*EGL|EGL_VERSION|gbm|drm|GL_' | head -40 || echo "  (no errors in debug validate)"
echo "== [8b] swaymsg reload (reproduces boot config load) =="
echo "  swaynag_count_before=$(pgrep -c -x swaynag 2>/dev/null || echo 0)"
runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 SWAYSOCK="${SWAYSOCK}" swaymsg reload 2>&1 | head -30
echo "  reload rc=$? (0 = clean)"
sleep 2
echo "  swaynag_count_after=$(pgrep -c -x swaynag 2>/dev/null || echo 0)  (after>before = reload re-triggered swaynag)"
echo "== [8c] swaynag parent + journal sway errors =="
ps -o pid,ppid,args -p $(pgrep -x swaynag | head -1) 2>/dev/null || echo "  (no swaynag)"
journalctl -b --no-pager 2>/dev/null | grep -iE 'sway.*(error|config)|config.*sway' | tail -12 || echo "  (no journal sway errors)"
echo "== [8d] sway's OWN parsed config errors (get_config) =="
runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 SWAYSOCK="${SWAYSOCK}" swaymsg -t get_config 2>/dev/null | python3 -c "import json,sys
try:
    d=json.load(sys.stdin)
    cfg=d.get('config','')
    errs=d.get('errors',[])
    print(f'  config_len={len(cfg)} errors={len(errs)}')
    for e in errs: print('  ERR:', e)
    if not errs: print('  (no recorded config errors)')
except Exception as ex: print('  PARSE_FAIL:', ex)"
echo "== [8e] sway BOOT stderr log (captured via .bashrc redirect) =="
if [ -f /home/diad/.diad/sway-boot.log ]; then
  echo "  log bytes: $(wc -c < /home/diad/.diad/sway-boot.log)"
  echo "  -- swaynag-trigger lines (Overwriting/Warning on line/Unable):"
  grep -inE 'Overwriting|Warning on line|Warning \(|Unable to|not recognized|already|deferred|unknown|failed to|error on line' /home/diad/.diad/sway-boot.log | grep -viE 'libEGL|eglInitialize|EGL_NOT|eglQueryDevice|kms_swrast|falling back|wlr.*EGL|EGL_VERSION|gbm|drm|GL_|layer surface|Read line|Config stored|unable to lock|seatd|libseat' | head -30 || echo "  (no swaynag-trigger lines)"
  echo "  -- all INFO/WARN/ERROR sway lines:"
  grep -iE '\[INFO\]|\[WARN|\[ERROR\]' /home/diad/.diad/sway-boot.log | grep -viE 'libEGL|eglInitialize|EGL_NOT|eglQueryDevice|kms_swrast|falling back|wlr.*EGL|EGL_VERSION|gbm|drm|GL_|layer surface|Read line|Config stored|swaybg|unable to lock|seatd|libseat|Config command|Performing config' | head -40 || echo "  (none)"
  echo "  -- last 15 lines of boot stderr:"
  tail -n 15 /home/diad/.diad/sway-boot.log
else
  echo "  (no sway-boot.log — .bashrc redirect not active)"
fi
echo "== [8f] swaynag detailed message (read from swaynag stdin pipe) =="
SNAGPID=$(pgrep -x swaynag | head -1)
echo "  swaynag_pid=$SNAGPID"
timeout 3 cat /proc/${SNAGPID}/fd/0 2>/dev/null | head -30 || echo "  (cannot read swaynag stdin)"
echo "== [8g] swaynag respawn isolation (kill, then reload, watch) =="
SNAGB=$(pgrep -x swaynag | head -1)
echo "  swaynag_before_kill=$SNAGB"
pkill -x swaynag; sleep 2
echo "  after_kill: $(pgrep -a swaynag 2>/dev/null || echo none)  (respawn-without-reload?)"
runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 SWAYSOCK="${SWAYSOCK}" swaymsg reload >/dev/null 2>&1
sleep 3
echo "  after_reload: $(pgrep -a swaynag 2>/dev/null || echo NONE-GONE)  (reload respawns it?)"
echo "== [9] waybar manual run with REAL socket (stderr, 8s) =="
timeout 8 runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 SWAYSOCK="${SWAYSOCK}" waybar -c /etc/waybar/config -s /etc/waybar/style.css 2>&1 | head -12
echo "  waybar rc=${PIPESTATUS[0]} (124 = stayed alive = GOOD)"
echo "== [10] supervisor env + waybar watchdog check =="
echo "  -- supervisor environ (WAYLAND/DISPLAY/SWAY):"
tr '\0' '\n' < /proc/$(pgrep -f diad-supervisor.sh | head -1)/environ 2>/dev/null | grep -E 'WAYLAND|XDG|DISPLAY|SWAY' || echo "  (cannot read supervisor environ)"
echo "  -- waybar process now:"
pgrep -a -x waybar || echo "  (no waybar process)"
echo "  -- journal waybar errors:"
journalctl -b --no-pager 2>/dev/null | grep -iE 'waybar|wl_display' | tail -8 || echo "  (no journal waybar entries)"
echo "== [11] boot timing (invariant 1: <7s budget) =="
systemd-analyze time 2>/dev/null || echo "  (systemd-analyze failed)"
echo "  -- critical chain to multi-user:"
systemd-analyze critical-chain multi-user.target 2>/dev/null | head -15
echo "  -- blame top-10:"
systemd-analyze blame 2>/dev/null | head -10
echo "  uptime now: $(uptime -p)"
echo "== [12] sudo surface (least privilege) =="
sudo -l -U diad 2>/dev/null | grep -E 'NOPASSWD' | head -25
echo "  -- forbidden probes (expect: not allowed):"
runuser -u diad -- sudo -n /bin/sh -c 'id' 2>&1 | head -2
runuser -u diad -- sudo -n /usr/bin/pacman -S --noconfirm -- /tmp/nonexistent.pkg.tar.zst 2>&1 | head -2
runuser -u diad -- sudo -n /usr/bin/kill -9 999999 2>&1 | head -2
echo "  -- wrapper rejections (expect rc=2 each):"
runuser -u diad -- sudo -n /usr/local/bin/diad-pacman install '../evil' 2>&1 | head -1; echo "    diad-pacman path-arg rc=$?"
runuser -u diad -- sudo -n /usr/local/bin/diad-killpid abc 2>&1 | head -1; echo "    diad-killpid non-numeric rc=$?"
runuser -u diad -- sudo -n /usr/local/bin/diad-killpid 1 2>&1 | head -1; echo "    diad-killpid pid1 rc=$?"
runuser -u diad -- sudo -n /usr/local/bin/diad-format /dev/sda ext4 2>&1 | head -1; echo "    diad-format without confirm rc=$?"
echo "== [13] control-plane origin gate (P0 remediation) =="
echo "  REST no-origin:    $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7860/api/v1/status) (expect 200)"
echo "  REST local-origin: $(curl -s -o /dev/null -w '%{http_code}' -H 'Origin: http://127.0.0.1:7860' http://127.0.0.1:7860/api/v1/status) (expect 200)"
echo "  REST evil-origin:  $(curl -s -o /dev/null -w '%{http_code}' -H 'Origin: http://evil.example' http://127.0.0.1:7860/api/v1/status) (expect 403)"
echo "  WS no-origin:      $(curl -s -m 5 -o /dev/null -w '%{http_code}' -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==' http://127.0.0.1:7860/ws) (expect 101)"
echo "  WS local-origin:   $(curl -s -m 5 -o /dev/null -w '%{http_code}' -H 'Origin: http://127.0.0.1:7860' -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==' http://127.0.0.1:7860/ws) (expect 101)"
echo "  WS evil-origin:    $(curl -s -m 5 -o /dev/null -w '%{http_code}' -H 'Origin: http://evil.example' -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==' http://127.0.0.1:7860/ws) (expect 403)"
echo "== [14] firewall & listeners =="
nft list ruleset 2>/dev/null | grep -E 'policy|dport|iif|accept' | head -8 || echo "  (nftables empty/absent — firewall NOT active)"
echo "  -- listening sockets:"
ss -tlnp 2>/dev/null | head -12
echo "== [15] memory footprint (tier budgets) =="
free -m
echo "  -- top RSS:"
ps -eo rss,comm --sort=-rss | head -12 | awk 'NR==1{print;next}{printf "%.0f MB  %s\n", $1/1024, $2}'
echo "  -- perf tier:"
cat /etc/diad/perf-tier 2>/dev/null; head -c 400 /etc/diad/hw-profile.json 2>/dev/null; echo
echo "== [16] media stack (VA-API) =="
vainfo 2>&1 | head -5
echo "  -- gst vaapi/libav elements:"
gst-inspect-1.0 2>/dev/null | grep -iE 'vaapi|libav' | head -6
echo "  -- DRI drivers:"
ls /usr/lib/dri 2>/dev/null | head -8
echo "== [17] app container isolation =="
grep -E 'profile=|application-mode' /usr/local/bin/diad-telegram /usr/local/bin/diad-slack 2>/dev/null | head -6
echo "  -- diad-apps state dir:"
ls -la /home/diad/.local/share/diad-apps 2>/dev/null || echo "  (not created yet — spawned on first launch)"
echo "== [18] workspace routing live check =="
D swaymsg -t get_tree 2>/dev/null | python3 -c "
import json,sys
t=json.load(sys.stdin)
def leaves(n):
    kids=n.get('nodes',[])+n.get('floating_nodes',[])
    return [n] if not kids else [l for k in kids for l in leaves(k)]
def walk(n):
    if n.get('type')=='workspace':
        wins=[(l.get('app_id'),l.get('name')) for l in leaves(n)]
        print(f\"  WS[{n.get('name')}] -> {wins}\")
    for c in n.get('nodes',[])+n.get('floating_nodes',[]): walk(c)
for c in t.get('nodes',[]): walk(c)
"
echo VFY_DONE_$((0))
VFY
bash /tmp/vfy.sh"""

PERSIST_SCRIPT = r"""cat > /tmp/persist.sh <<'PST'
#!/bin/bash
echo "== [P0] block devices =="
lsblk -o NAME,SIZE,LABEL,MOUNTPOINT | head -8
echo "== [P1] create DIAD-DATA on first non-cdrom disk =="
DEV=$(lsblk -dnpo NAME | grep -vE "sr0|loop" | head -1)
echo "  using $DEV"
parted -s "$DEV" mklabel gpt mkpart primary 1MiB 100% && echo "  parted ok" || echo "  PARTED_FAIL"
udevadm settle
P1="${DEV}1"
mkfs.ext4 -F -L DIAD-DATA "$P1" && echo "  mkfs ok" || echo "  MKFS_FAIL"
udevadm settle
echo "== [P2] diad-storage-setup.sh as diad (sudo mount path) =="
runuser -u diad -- env HOME=/home/diad bash /usr/local/bin/diad-storage-setup.sh 2>&1 | tail -4
findmnt -rn /home/diad/data >/dev/null 2>&1 && echo "  MOUNTED_BY_SCRIPT" || echo "  NOT_MOUNTED_BY_SCRIPT (sudoers gap?)"
echo "== [P3] root mount fallback + marker persistence across umount =="
mount LABEL=DIAD-DATA /home/diad/data 2>&1 && echo "  root mount ok"
chown 1000:1000 /home/diad/data
runuser -u diad -- sh -c 'echo MARK42 > /home/diad/data/persist-test.txt' && echo "  marker written"
umount /home/diad/data
mount LABEL=DIAD-DATA /home/diad/data 2>/dev/null
cat /home/diad/data/persist-test.txt 2>/dev/null || echo "  MARKER_LOST"
echo "== [P4] fstab wiring for boot-time mount =="
grep -r DIAD-DATA /etc/fstab /etc/fstab.d 2>/dev/null || echo "  (no fstab entry for DIAD-DATA — boot mount relies on storage-setup only)"
echo PERSIST_DONE_$((0))
PST
bash /tmp/persist.sh"""

DIAG_SCRIPT = r"""cat > /tmp/diag.sh <<'DGS'
#!/bin/bash
echo "== [D1] failed units root cause =="
for u in systemd-oomd.service systemd-resolved.service systemd-sysusers.service systemd-timesyncd.service avahi-daemon.service krb5-kdc.service lm_sensors.service; do
  echo "-- $u"
  systemctl status "$u" --no-pager -l 2>/dev/null | grep -E "Active:|Process:|Error|error" | head -4
done
echo "== [D2] bloat enabled state =="
for u in sshd.service avahi-daemon.service krb5-kdc.service systemd-journal-upload.service gpsd.socket sudo_logsrvd.service git.socket; do
  echo "  $u: $(systemctl is-enabled $u 2>&1)"
done
echo "== [D3] mako manual run (3s) =="
runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 timeout 3 mako 2>&1 | head -6
echo "  mako rc=${PIPESTATUS[0]} (124=alive, other=crash)"
echo "== [D4] journal errors this boot =="
journalctl -b -p err --no-pager -q 2>/dev/null | head -20
echo "== [D5] power-setup sudo usage =="
grep -n "sudo" /usr/local/bin/diad-power-setup.sh | head -8
echo DIAG_DONE_$((0))
DGS
bash /tmp/diag.sh"""


def build_inject():
    """Initramfs postmount-break injection block (runs as root; /new_root is
    the writable overlay root). Sends getty autologin + matrix-shell service
    AND base64-heredocs of the FIXED source files (sway config, supervisor,
    style.css) so the matrix verifies fixes end-to-end without a rebuild."""
    cmds = [
        "echo INJ_START",
        # Pre-seed first-boot wizards' completion markers so the harness VM
        # boots straight into the session (mirrors any second+ real boot).
        "mkdir -p /new_root/var/lib/diad && touch /new_root/var/lib/diad/identity-setup",
        "mkdir -p /new_root/home/diad/.diad && touch /new_root/home/diad/.diad/.first-run-complete",
        "chown -R 1000:1000 /new_root/home/diad/.diad",
        "mkdir -p /new_root/etc/systemd/system/getty@tty1.service.d",
        "printf '%s\\n' '[Service]' 'ExecStart=' "
        "'ExecStart=-/usr/bin/agetty --autologin diad --noclear tty1 38400 linux' "
        "> /new_root/etc/systemd/system/getty@tty1.service.d/autologin.conf",
        "printf '%s\\n' '#!/bin/sh' 'exec /bin/sh -i' "
        "> /new_root/usr/local/bin/matrix-shell.sh",
        "chmod 755 /new_root/usr/local/bin/matrix-shell.sh",
        "printf '%s\\n' '[Unit]' 'Description=Matrix root shell (ttyS1)' "
        "'DefaultDependencies=no' 'After=systemd-vconsole-setup.service' "
        "'[Service]' 'ExecStart=/usr/local/bin/matrix-shell.sh' "
        "'StandardInput=tty' 'StandardOutput=tty' 'StandardError=tty' "
        "'TTYPath=/dev/ttyS1' 'TTYReset=yes' 'TTYVHangup=yes' "
        "'KillMode=process' 'IgnoreSIGPIPE=no' "
        "'[Install]' 'WantedBy=sysinit.target' "
        "> /new_root/etc/systemd/system/matrix-shell.service",
        "mkdir -p /new_root/etc/systemd/system/sysinit.target.wants",
        "ln -sf ../matrix-shell.service "
        "/new_root/etc/systemd/system/sysinit.target.wants/matrix-shell.service",
    ]
    for dst in sorted(FIX_SOURCES):
        src = FIX_SOURCES[dst]
        with open(src, "rb") as f:
            b64 = base64.b64encode(f.read()).decode()
        cmds.append(f"base64 -d > {dst} <<'MATRIXB64'")
        for j in range(0, len(b64), 76):
            cmds.append(b64[j:j + 76])
        cmds.append("MATRIXB64")
    cmds.append("echo ---INJECTED-FILES---")
    cmds.append("ls -l /new_root/etc/systemd/system/getty@tty1.service.d/autologin.conf "
                "/new_root/usr/local/bin/matrix-shell.sh "
                "/new_root/etc/systemd/system/matrix-shell.service "
                "/new_root/etc/systemd/system/sysinit.target.wants/matrix-shell.service "
                + " ".join(sorted(FIX_SOURCES)))
    cmds.append("echo INJ_OK")
    cmds.append("exit")
    return "\n".join(cmds) + "\n"


def steps_break():
    # postmount wait is longer for 3-ram (copytoram copies the airootfs to RAM)
    to = 600 if ENTRY == "3-ram" else 300
    return [
        ("connect", CON_SOCK),
        ("expect", r"Post-mount break requested", to),
        ("send", build_inject()),
        ("expect", "INJ_OK", 60),
        ("close",),
    ]


def steps_common():
    to = 600 if ENTRY == "3-ram" else 300
    return [
        # CRITICAL: wait for the bash prompt before sending ANYTHING. Bytes sent
        # before bash/readline is up get corrupted by the tty reset + bracketed
        # paste handshake (observed: mangled echo + stuck quote continuation).
        ("expect", r"sh-\d+\.\d+[#$] ", to),
        ("send", "echo P_READY_MARK"),
        ("expect", "P_READY_MARK", 30),
        ("send", "echo ===CMDLINE===; cat /proc/cmdline; echo MARK_CMDLINE"),
        ("expect", "MARK_CMDLINE", 30),
        ("send", "echo ===MOUNT===; mount | grep -E 'erofs|archiso'; echo MARK_MOUNT"),
        ("expect", "MARK_MOUNT", 30),
        ("send", "echo ===FAILED===; systemctl --failed --no-legend; echo MARK_FAILED"),
        ("expect", "MARK_FAILED", 30),
        ("send", "echo ===GETTY===; pgrep -a agetty; echo MARK_GETTY"),
        ("expect", "MARK_GETTY", 30),
    ]


def steps_session():
    # autologin was already injected at boot (getty@tty1 drop-in); settle long
    # enough for the supervisor to reach steady state (waybar/mako watchdogs).
    return [
        ("sleep", 90),
        ("send", VFY_SCRIPT),
        ("expect", "VFY_DONE_0", 300),
    ]


def steps_ram_extra():
    return [
        ("send", "echo ===MEM===; grep -E 'MemTotal|MemAvailable' /proc/meminfo; echo MARK_MEM"),
        ("expect", "MARK_MEM", 30),
    ]


def run_keys(sess, logf):
    """Interactive keybinding round: real input via QEMU monitor sendkey,
    assertions via the ttyS1 root shell. Values are parsed from the
    transcript afterwards; markers only pace the session."""
    mon = connect(MON_SOCK, 30)
    ms = Session(mon, logf)
    ms.expect(r"\(qemu\)", 15)

    sess.send("cat > /tmp/kq.py <<'KQ'\n"
              "import json,sys\n"
              "t=json.load(sys.stdin)\n"
              "s=[t];fs=[]\n"
              "while s:\n"
              " n=s.pop();s+=n.get('nodes',[])+n.get('floating_nodes',[])\n"
              " if n.get('focused'):fs.append((n.get('name'),n.get('fullscreen_mode')))\n"
              "print('FULLSCREEN='+str(fs))\n"
              "KQ")
    sess.sleep(1)

    def key(k, settle=3.0):
        ms.send(f"sendkey {k}")
        ms.expect(r"\(qemu\)", 10)
        time.sleep(settle)

    def q(cmd, mark, to=60):
        sess.send(f"{cmd}; echo {mark}$((0))")
        sess.expect(f"{mark}0", to)

    SW = "SWAYSOCK=$(ls -t /run/user/1000/sway-ipc.*.sock | head -1)"
    WSF = (f"runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 {SW} "
           "swaymsg -t get_workspaces 2>/dev/null | python3 -c "
           "\"import json,sys;ws=json.load(sys.stdin);"
           "print('FOCUSED='+[w['name'] for w in ws if w.get('focused')][0])\"")
    WSFULL = (f"runuser -u diad -- env XDG_RUNTIME_DIR=/run/user/1000 {SW} "
              "swaymsg -t get_tree 2>/dev/null | python3 /tmp/kq.py")

    q("echo KEYTEST_START", "MK_START")
    q(WSF, "MK_BASE")
    key("f2"); q(WSF, "MK_F2")
    key("f3"); q(WSF, "MK_F3")
    key("f1"); q(WSF, "MK_F1")
    key("meta_l-shift-l", 4)
    q("pgrep -fc '7860/launch' || echo 0", "MK_LAUNCH")
    key("meta_l-t", 5)
    q("pgrep -af chromium | grep -c telegram || echo 0", "MK_TG")
    q("pgrep -af chromium | grep telegram | grep -cE 'user-data-dir|--profile' "
      "|| echo NO-ISOLATION-FLAG", "MK_TGISO")
    key("meta_l-q", 2)
    q("pgrep -af chromium | grep -c telegram || echo 0", "MK_TGCLOSED")
    key("meta_l-f", 2)
    q(WSFULL, "MK_FULL")
    key("meta_l-l", 2)
    q("pgrep -c -x swaylock || echo 0", "MK_LOCK")
    q("pkill -x swaylock; echo unlocked", "MK_UNLOCK")
    # Post-fix verification: launchpad has been polling for ~20s by now.
    time.sleep(8)
    q("grep -c 'rejected request without valid API token' "
      "/home/diad/.diad/daemon.log || echo 0", "MK_WARNCOUNT")
    q("ls /etc/systemd/system | grep -cE 'debug-serial|serial-getty' || echo 0",
      "MK_UNITS")
    q("pgrep -cx sway && pgrep -cx waybar && pgrep -cx diad-agent", "MK_DESKTOP")
    mon.close()


def run():
    qargs = build_qemu(ENTRY)
    qerr = open(QEMU_ERR, "wb")
    print(f"[matrix] {ENTRY} booting (mem={MEM})...", flush=True)
    qemu = subprocess.Popen(qargs, stdout=subprocess.DEVNULL, stderr=qerr)

    logf = open(f"{BASE}/transcript-{ENTRY}.log", "w", encoding="utf-8")
    con = None
    dbg = None
    sess = None

    try:
        for step in steps_break():
            kind, *rest = step
            if kind == "connect":
                con = connect(rest[0], 60)
                sess = Session(con, logf)
            elif kind == "expect":
                sess.expect(rest[0], rest[1])
            elif kind == "send":
                sess.send(rest[0])
            elif kind == "close":
                con.close()

        dbg = connect(DBG_SOCK, 240)
        sess = Session(dbg, logf)

        for step in steps_common():
            kind, *rest = step
            if kind == "send":
                sess.send(rest[0])
            elif kind == "expect":
                sess.expect(rest[0], rest[1])
            elif kind == "sleep":
                sess.sleep(rest[0])

        if FULL_SESSION or ENTRY in ("q4-persist", "q5-keys", "q6-verbose"):
            for step in steps_session():
                kind, *rest = step
                if kind == "send":
                    sess.send(rest[0])
                elif kind == "expect":
                    sess.expect(rest[0], rest[1])
                elif kind == "sleep":
                    sess.sleep(rest[0])

        if ENTRY == "3-ram":
            for step in steps_ram_extra():
                kind, *rest = step
                if kind == "send":
                    sess.send(rest[0])
                else:
                    sess.expect(rest[0], rest[1])

        if ENTRY == "q4-persist":
            sess.send(PERSIST_SCRIPT)
            sess.expect("PERSIST_DONE_0", 240)
        if ENTRY == "q6-verbose":
            sess.send(DIAG_SCRIPT)
            sess.expect("DIAG_DONE_0", 240)
        if ENTRY == "q5-keys":
            run_keys(sess, logf)

        # screenshot for the full-session entry
        if FULL_SESSION:
            try:
                mon = connect(MON_SOCK, 30)
                ms = Session(mon, logf)
                ms.expect(r"\(qemu\)", 15)
                ms.send(f"screendump {SHOT_PPM}")
                ms.expect(r"\(qemu\)", 20)
                mon.close()
                if os.path.exists(SHOT_PPM):
                    from PIL import Image
                    Image.open(SHOT_PPM).convert("RGB").save(SHOT_PNG)
                    print(f"[matrix] screenshot -> {SHOT_PNG}", flush=True)
            except Exception as e:
                print(f"[matrix] screendump skipped: {e}", flush=True)

        print("[matrix] checks done", flush=True)

        if not NO_POWEROFF:
            sess.send("poweroff")
            end = time.time() + 60
            while time.time() < end:
                if qemu.poll() is not None:
                    break
                sess._drain(2.0)
            if qemu.poll() is None:
                print("[matrix] guest did not power off; killing qemu", flush=True)
                qemu.terminate()
                try:
                    qemu.wait(10)
                except subprocess.TimeoutExpired:
                    qemu.kill()
    except TimeoutError as e:
        print(f"[matrix] FAIL: {e}", flush=True)
        qemu.kill()
        qemu.wait()
        qerr.close()
        sys.exit(1)
    finally:
        logf.close()
        qerr.close()

    print(f"[matrix] {ENTRY} OK (qemu rc={qemu.returncode})", flush=True)


if __name__ == "__main__":
    run()
