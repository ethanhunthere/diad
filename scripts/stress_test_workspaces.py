#!/usr/bin/env python3
"""
DIAD OS — Multi-Window Multi-Workspace Stress & Responsiveness Audit Suite
Connects via QMP socket and runs deep stress tests:
1. 1 Window (DIAD Agent Studio)
2. 4 Windows on Workspace 1 (Tiled)
3. 8 Windows on Workspace 1 (Tiled & Tabbed modes)
4. Interactive typing & command execution across multiple windows
5. Workspace 2 (Single Browser Window)
6. Workspace 3 (2 Windows: Telegram + Slack Side-by-Side)
"""
import socket
import json
import time
import sys
import subprocess

def connect_qmp():
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    for _ in range(30):
        try:
            s.connect("/tmp/qmp.sock")
            break
        except Exception:
            time.sleep(1)
    else:
        print("Failed to connect to /tmp/qmp.sock")
        sys.exit(1)
    s.recv(2048)
    
    def cmd(execute, arguments=None):
        req = {"execute": execute}
        if arguments:
            req["arguments"] = arguments
        s.sendall(json.dumps(req).encode() + b"\r\n")
        while True:
            line = s.makefile("rb").readline()
            if not line:
                return None
            data = json.loads(line.decode())
            if "return" in data or "error" in data:
                return data
    
    cmd("qmp_capabilities")
    return s, cmd

def send_key(cmd, key_name):
    cmd("send-key", {"keys": [{"type": "qcode", "data": key_name}]})
    time.sleep(0.05)

def send_combo(cmd, keys):
    event_keys = [{"type": "qcode", "data": k} for k in keys]
    cmd("send-key", {"keys": event_keys})
    time.sleep(0.08)

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
        else:
            send_key(cmd, ch.lower())
        time.sleep(0.02)

def screendump(cmd, out_ppm, out_png):
    cmd("screendump", {"filename": out_ppm})
    time.sleep(0.2)
    subprocess.run(["magick", out_ppm, out_png], check=True)
    print(f"Saved: {out_png}")

def main():
    print("Waiting 35s for VM desktop to initialize...")
    time.sleep(35)
    
    s, cmd = connect_qmp()
    print("Connected to live VM QMP interface!")
    
    # 1. Capture pristine Workspace 1 (1 window)
    print("\n--- PHASE 1: 1 Window on Workspace 1 ---")
    send_key(cmd, "f1")
    time.sleep(1)
    screendump(cmd, "/tmp/stress_1win.ppm", "/tmp/stress_1win.png")
    
    # 2. Open 3 more windows (Total 4 windows)
    print("\n--- PHASE 2: 4 Windows on Workspace 1 ---")
    for i in range(3):
        send_combo(cmd, ["meta_l", "ret"])
        time.sleep(0.6)
    time.sleep(1)
    screendump(cmd, "/tmp/stress_4win_tiled.ppm", "/tmp/stress_4win_tiled.png")
    
    # 3. Open 4 more windows (Total 8 windows)
    print("\n--- PHASE 3: 8 Windows on Workspace 1 ---")
    for i in range(4):
        send_combo(cmd, ["meta_l", "ret"])
        time.sleep(0.6)
    time.sleep(1)
    screendump(cmd, "/tmp/stress_8win_tiled.ppm", "/tmp/stress_8win_tiled.png")
    
    # Run commands in active terminal window
    type_text(cmd, "diad status\n")
    time.sleep(1.5)
    screendump(cmd, "/tmp/stress_8win_active.ppm", "/tmp/stress_8win_active.png")
    
    # 4. Switch to Tabbed layout (Super+w)
    print("\n--- PHASE 4: 8 Windows in Tabbed Mode (Super+w) ---")
    send_combo(cmd, ["meta_l", "w"])
    time.sleep(1)
    screendump(cmd, "/tmp/stress_8win_tabbed.ppm", "/tmp/stress_8win_tabbed.png")
    
    # Switch tabs with Super+Left / Super+Right
    send_combo(cmd, ["meta_l", "right"])
    time.sleep(0.5)
    screendump(cmd, "/tmp/stress_8win_tab_focused.ppm", "/tmp/stress_8win_tab_focused.png")
    
    # 5. Workspace 2 (1 window: DIAD Browser)
    print("\n--- PHASE 5: Workspace 2 (Browser) ---")
    send_key(cmd, "f2")
    time.sleep(1.5)
    screendump(cmd, "/tmp/stress_ws2_browser.ppm", "/tmp/stress_ws2_browser.png")
    
    # 6. Workspace 3 (2 windows: Telegram + Slack)
    print("\n--- PHASE 6: Workspace 3 (Comms) ---")
    send_key(cmd, "f3")
    time.sleep(1.5)
    screendump(cmd, "/tmp/stress_ws3_comms.ppm", "/tmp/stress_ws3_comms.png")
    
    s.close()
    print("\nStress audit completed successfully!")

if __name__ == "__main__":
    main()
