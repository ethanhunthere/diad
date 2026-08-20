#!/usr/bin/env bash
set -euo pipefail

# Kill any stale QEMU
pkill -9 -f qemu-system-x86_64 2>/dev/null || true
rm -f /tmp/qmp.sock /tmp/qmp-diad.sock

# Start run-vm.sh in background
./scripts/run-vm.sh &
VM_PID=$!
echo "[AUDIT] Started VM with PID: $VM_PID"

# Run python audit after waiting for boot
python3 - << 'EOF'
import socket
import json
import time
import subprocess
import os

ARTIFACT_DIR = "/home/f0id/.gemini/antigravity-cli/brain/4505434e-5114-452d-b8ef-610b6bdca0a8"

def connect_qmp():
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    for _ in range(50):
        try:
            s.connect("/tmp/qmp.sock")
            break
        except Exception:
            time.sleep(1)
    else:
        print("Failed to connect to /tmp/qmp.sock")
        return None, None
    s.recv(4096)
    
    def cmd(execute, arguments=None):
        req = {"execute": execute}
        if arguments:
            req["arguments"] = arguments
        s.sendall(json.dumps(req).encode() + b"\r\n")
        time.sleep(0.1)
        while True:
            line = s.makefile("rb").readline()
            if not line:
                return None
            try:
                data = json.loads(line.decode())
                if "return" in data or "error" in data:
                    return data
            except Exception:
                pass
    
    cmd("qmp_capabilities")
    return s, cmd

def send_key(cmd, key_name):
    cmd("send-key", {"keys": [{"type": "qcode", "data": key_name}]})
    time.sleep(0.15)

def send_combo(cmd, keys):
    event_keys = [{"type": "qcode", "data": k} for k in keys]
    cmd("send-key", {"keys": event_keys})
    time.sleep(0.2)

def type_text(cmd, text):
    for ch in text:
        if ch == "\n":
            send_key(cmd, "ret")
        elif ch == " ":
            send_key(cmd, "spc")
        elif ch == "-":
            send_key(cmd, "minus")
        elif ch == "_":
            send_combo(cmd, ["shift", "minus"])
        elif ch == ".":
            send_key(cmd, "dot")
        elif ch == "/":
            send_key(cmd, "slash")
        elif ch.isupper():
            send_combo(cmd, ["shift", ch.lower()])
        else:
            send_key(cmd, ch.lower())
        time.sleep(0.04)

def screendump(cmd, filename):
    ppm = f"/tmp/{filename}.ppm"
    png = os.path.join(ARTIFACT_DIR, f"{filename}.png")
    cmd("screendump", {"filename": ppm})
    time.sleep(0.4)
    if os.path.exists(ppm):
        subprocess.run(["magick", ppm, png], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if os.path.exists(png):
            try:
                os.remove(ppm)
            except Exception:
                pass
            print(f"[AUDIT] Saved -> {png}")
            return png
    return None

print("[AUDIT] Waiting 36s for live VM desktop to initialize...")
time.sleep(36)

s, cmd = connect_qmp()
if not s:
    exit(1)

print("[AUDIT] Connected to QMP! Starting visual captures...")

# 1. Workspace 1
send_key(cmd, "f1")
time.sleep(1.5)
screendump(cmd, "audit_final_ws1_agent")

# 2. Workspace 2 (Browser with official DIAD logo)
print("[AUDIT] Switching to Workspace 2 (F2)...")
send_key(cmd, "f2")
time.sleep(3.5)
screendump(cmd, "audit_final_ws2_browser_official_logo")

# 3. Workspace 3 (Comms)
print("[AUDIT] Switching to Workspace 3 (F3)...")
send_key(cmd, "f3")
time.sleep(2.5)
screendump(cmd, "audit_final_ws3_comms")

# 4. Workspace 1 Split-screen Terminal with 11pt font
print("[AUDIT] Switching to Workspace 1 (F1) and opening Terminal...")
send_key(cmd, "f1")
time.sleep(1.0)
send_combo(cmd, ["meta_l", "ret"])
time.sleep(2.0)
type_text(cmd, "diad status\n")
time.sleep(2.0)
screendump(cmd, "audit_final_terminal_11pt")

# 5. Launch TUI inside the split terminal
print("[AUDIT] Launching TUI inside split terminal...")
type_text(cmd, "diad-agent --tui\n")
time.sleep(3.0)
screendump(cmd, "audit_final_tui_split_responsive")

# 6. Lockscreen
print("[AUDIT] Testing Lockscreen (Super + Shift + P)...")
send_combo(cmd, ["meta_l", "shift", "p"])
time.sleep(3.0)
screendump(cmd, "audit_final_lockscreen_official_logo")

# 7. Unlock
print("[AUDIT] Unlocking...")
send_key(cmd, "spc")
time.sleep(1.5)
screendump(cmd, "audit_final_unlocked_desktop")

print("[AUDIT] All visual captures complete!")
s.close()
EOF

pkill -9 -f qemu-system-x86_64 2>/dev/null || true
echo "[AUDIT] Done!"
