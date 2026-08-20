#!/usr/bin/env python3
import socket
import json
import time
import os
import subprocess

ARTIFACT_DIR = "/home/f0id/.gemini/antigravity-cli/brain/4505434e-5114-452d-b8ef-610b6bdca0a8"
QMP_SOCK = "/tmp/qmp.sock"

def connect():
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    for _ in range(50):
        try:
            s.connect(QMP_SOCK)
            break
        except Exception:
            time.sleep(1)
    else:
        print("Could not connect to QMP")
        return None
    s.recv(4096)
    s.sendall(json.dumps({"execute": "qmp_capabilities"}).encode() + b"\r\n")
    s.recv(4096)
    return s

def cmd(s, execute, arguments=None):
    req = {"execute": execute}
    if arguments:
        req["arguments"] = arguments
    s.sendall(json.dumps(req).encode() + b"\r\n")
    time.sleep(0.15)
    return s.recv(4096)

def key(s, keyname):
    cmd(s, "send-key", {"keys": [{"type": "qcode", "data": keyname}]})
    time.sleep(0.2)

def combo(s, keys):
    cmd(s, "send-key", {"keys": [{"type": "qcode", "data": k} for k in keys]})
    time.sleep(0.25)

def type_text(s, text):
    for ch in text:
        if ch == ' ':
            key(s, "spc")
        elif ch == '\n':
            key(s, "ret")
        elif ch == '-':
            key(s, "minus")
        elif ch == '/':
            key(s, "slash")
        elif ch == '.':
            key(s, "dot")
        elif ch.isupper():
            combo(s, ["shift", ch.lower()])
        else:
            key(s, ch)
        time.sleep(0.04)

def screenshot(s, name):
    ppm = f"/tmp/{name}.ppm"
    png = os.path.join(ARTIFACT_DIR, f"{name}.png")
    cmd(s, "screendump", {"filename": ppm})
    time.sleep(0.5)
    if os.path.exists(ppm):
        subprocess.run(["magick", ppm, png], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if os.path.exists(png):
            try:
                os.remove(ppm)
            except Exception:
                pass
            print(f"Captured -> {png}")
            return png
    return None

def main():
    s = connect()
    if not s:
        return

    print("Connected to QMP. Waiting 18s for full Sway autostart...")
    time.sleep(18)

    # 1. Capture Workspace 1
    screenshot(s, "audit_v2_ws1_agent")

    # 2. Switch to Workspace 2 (Browser with official DIAD logo)
    print("Switching to Workspace 2 (F2)...")
    key(s, "f2")
    time.sleep(3.0)
    screenshot(s, "audit_v2_ws2_browser_official_logo")

    # 3. Switch to Workspace 3 (Comms)
    print("Switching to Workspace 3 (F3)...")
    key(s, "f3")
    time.sleep(2.5)
    screenshot(s, "audit_v2_ws3_comms")

    # 4. Open Terminal and run diad status
    print("Switching back to Workspace 1 (F1) and launching terminal...")
    key(s, "f1")
    time.sleep(1.0)
    combo(s, ["meta_l", "ret"])
    time.sleep(2.0)
    type_text(s, "diad status\n")
    time.sleep(2.0)
    screenshot(s, "audit_v2_terminal_crimson")

    # 5. Lockscreen
    print("Testing lockscreen...")
    type_text(s, "diad lock\n")
    time.sleep(3.0)
    screenshot(s, "audit_v2_lockscreen_official_logo")

    # 6. Unlock
    print("Unlocking...")
    key(s, "spc")
    time.sleep(1.5)
    screenshot(s, "audit_v2_unlocked_final")

    s.close()
    print("Audit capture complete!")

if __name__ == "__main__":
    main()
