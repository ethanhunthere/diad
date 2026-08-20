#!/usr/bin/env python3
"""
DIAD OS — Multi-Window & Multi-Workspace Deep Responsiveness Audit Suite
"""
import socket
import json
import time
import sys
import subprocess

def connect_qmp():
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    for _ in range(40):
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
    time.sleep(0.06)

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
        time.sleep(0.03)

def screendump(cmd, out_ppm, out_png):
    cmd("screendump", {"filename": out_ppm})
    time.sleep(0.3)
    subprocess.run(["magick", out_ppm, out_png], check=True)
    print(f"Saved: {out_png}")

def main():
    print("Waiting 38s for live VM desktop to initialize...")
    time.sleep(38)
    
    s, cmd = connect_qmp()
    print("Connected to QMP socket!")
    
    # 1. Baseline: 1 Window on Workspace 1
    print("\n[STAGE 1] Workspace 1 (1 Window Baseline)")
    send_key(cmd, "f1")
    time.sleep(1)
    screendump(cmd, "/tmp/audit_ws1_1win.ppm", "/tmp/audit_ws1_1win.png")
    
    # 2. 4 Windows on Workspace 1
    print("\n[STAGE 2] Workspace 1 (4 Windows Tiled)")
    for _ in range(3):
        send_combo(cmd, ["meta_l", "ret"])
        time.sleep(0.6)
    time.sleep(1)
    screendump(cmd, "/tmp/audit_ws1_4win.ppm", "/tmp/audit_ws1_4win.png")
    
    # 3. 8 Windows on Workspace 1
    print("\n[STAGE 3] Workspace 1 (8 Windows Tiled Extreme Load)")
    for _ in range(4):
        send_combo(cmd, ["meta_l", "ret"])
        time.sleep(0.6)
    time.sleep(1)
    screendump(cmd, "/tmp/audit_ws1_8win_tiled.ppm", "/tmp/audit_ws1_8win_tiled.png")
    
    # Run command across terminal
    print("\n[STAGE 3.1] Running 'diad status' in 8-window layout")
    type_text(cmd, "diad status\n")
    time.sleep(1.5)
    screendump(cmd, "/tmp/audit_ws1_8win_active.ppm", "/tmp/audit_ws1_8win_active.png")
    
    # 4. 8 Windows Tabbed Mode (Super + w)
    print("\n[STAGE 4] Workspace 1 (8 Windows Tabbed Mode)")
    send_combo(cmd, ["meta_l", "w"])
    time.sleep(1)
    screendump(cmd, "/tmp/audit_ws1_8win_tabbed.ppm", "/tmp/audit_ws1_8win_tabbed.png")
    
    # Cycle tabs (Super + Right)
    send_combo(cmd, ["meta_l", "right"])
    time.sleep(0.5)
    screendump(cmd, "/tmp/audit_ws1_8win_tab2.ppm", "/tmp/audit_ws1_8win_tab2.png")
    
    # 5. Workspace 2 (1 Window: Browser)
    print("\n[STAGE 5] Workspace 2 (1 Window Browser)")
    send_key(cmd, "f2")
    time.sleep(1.5)
    screendump(cmd, "/tmp/audit_ws2_browser.ppm", "/tmp/audit_ws2_browser.png")
    
    # 6. Workspace 3 (2 Windows: Comms Launchpad)
    print("\n[STAGE 6] Workspace 3 (2 Windows Comms)")
    send_key(cmd, "f3")
    time.sleep(1.5)
    screendump(cmd, "/tmp/audit_ws3_comms.ppm", "/tmp/audit_ws3_comms.png")
    
    # 7. Rapid Workspace Switching
    print("\n[STAGE 7] Rapid Multi-Workspace Switching Test")
    for _ in range(3):
        send_key(cmd, "f1")
        time.sleep(0.2)
        send_key(cmd, "f2")
        time.sleep(0.2)
        send_key(cmd, "f3")
        time.sleep(0.2)
    send_key(cmd, "f1")
    time.sleep(0.5)
    screendump(cmd, "/tmp/audit_final_ws1.ppm", "/tmp/audit_final_ws1.png")
    
    s.close()
    print("\nComprehensive audit finished successfully!")

if __name__ == "__main__":
    main()
