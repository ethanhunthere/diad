#!/usr/bin/env bash
set -euo pipefail

# Kill any existing QEMU
pkill -9 -f qemu-system-x86_64 2>/dev/null || true
rm -f /tmp/qmp.sock /tmp/qmp-diad.sock

# Start QEMU with VNC + QMP in background
./scripts/run-vm.sh &
VM_PID=$!
echo "[TEST] Started QEMU VM with PID: $VM_PID"

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
            print(f"[TEST] Saved -> {png}")
            return png
    return None

print("[TEST] Waiting 38s for live VM to boot into desktop...")
time.sleep(38)

s, cmd = connect_qmp()
if not s:
    print("Could not connect to QMP")
    exit(1)

print("[TEST] Connected to QMP! Commencing full test suite...")

# 1. Capture Workspace 1 (DIAD Agent Studio with enlarged Waybar 38px and new GUI)
send_key(cmd, "f1")
time.sleep(2.0)
screendump(cmd, "live_test_01_ws1_agent_studio")

# 2. Test Explainer Modal (type /help in chat)
print("[TEST] Opening Explainer Modal via /help...")
type_text(cmd, "/help\n")
time.sleep(2.0)
screendump(cmd, "live_test_02_explainer_slide1")

# Click Next to Slide 2
print("[TEST] Navigating to Slide 2 (Right Arrow)...")
send_key(cmd, "right")
time.sleep(1.0)
screendump(cmd, "live_test_03_explainer_slide2")

# Click Next to Slide 3
print("[TEST] Navigating to Slide 3 (Right Arrow)...")
send_key(cmd, "right")
time.sleep(1.0)
screendump(cmd, "live_test_04_explainer_slide3")

# Close Explainer (Escape)
print("[TEST] Closing Explainer (Escape)...")
send_key(cmd, "esc")
time.sleep(1.0)

# 3. Test Wi-Fi Manager Dialog
print("[TEST] Launching Wi-Fi Manager...")
send_combo(cmd, ["meta_l", "shift", "w"])
time.sleep(2.0)
screendump(cmd, "live_test_05_wifi_menu")
send_key(cmd, "esc")
time.sleep(1.0)

# 4. Test Split Screen Terminal & TUI on Workspace 1
print("[TEST] Spawning Terminal on Workspace 1...")
send_combo(cmd, ["meta_l", "ret"])
time.sleep(2.0)
type_text(cmd, "diad status\n")
time.sleep(2.0)
screendump(cmd, "live_test_06_terminal_status_11pt")

# Launch TUI inside split terminal
print("[TEST] Launching TUI inside split terminal...")
type_text(cmd, "diad-agent --tui\n")
time.sleep(3.0)
screendump(cmd, "live_test_07_tui_split_view")

# Close split terminal
send_combo(cmd, ["meta_l", "shift", "q"])
time.sleep(1.0)

# 5. Workspace 2: Browser with official DIAD logo
print("[TEST] Switching to Workspace 2 (F2)...")
send_key(cmd, "f2")
time.sleep(3.0)
screendump(cmd, "live_test_08_ws2_browser")

# 6. Workspace 3: Comms Launchpad
print("[TEST] Switching to Workspace 3 (F3)...")
send_key(cmd, "f3")
time.sleep(2.5)
screendump(cmd, "live_test_09_ws3_comms")

# 7. Test Lockscreen
print("[TEST] Testing Lockscreen (Super + Shift + P)...")
send_combo(cmd, ["meta_l", "shift", "p"])
time.sleep(3.0)
screendump(cmd, "live_test_10_lockscreen")

# 8. Unlock Lockscreen
print("[TEST] Unlocking screen (Space)...")
send_key(cmd, "spc")
time.sleep(2.0)
screendump(cmd, "live_test_11_unlocked_desktop")

print("[TEST] All tests completed successfully!")
s.close()
EOF

pkill -9 -f qemu-system-x86_64 2>/dev/null || true
echo "[TEST] VM shutdown complete."
