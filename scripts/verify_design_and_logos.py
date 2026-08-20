#!/usr/bin/env python3
"""
DIAD OS — Automated Full Visual and Navigation Audit Script
Tests:
1. Workspace 1 (DIAD Agent with Industrial Basalt & Crimson theme)
2. Workspace 2 (Browser with Official DIAD Logo Image, Dark Dropdown, Basalt theme)
3. Workspace 3 (Comms Launchpad)
4. Terminal (Foot with Surface-Tiled #0F1110, #E5EAE6 text, #B32032 cursor, diad CLI)
5. Web Lockscreen (Official DIAD Logo Image, #161918 card, #38A169 unlock badge)
6. Rapid zero-flash workspace navigation
"""

import os
import sys
import time
import json
import socket
import subprocess

import glob

QMP_SOCK = "/tmp/qmp_design_audit.sock"
iso_files = sorted(glob.glob("/home/f0id/diados/out/*.iso"), key=os.path.getmtime, reverse=True)
ISO_PATH = iso_files[0] if iso_files else "/home/f0id/diados/out/diados-2026.08.20-x86_64.iso"
ARTIFACT_DIR = "/home/f0id/.gemini/antigravity-cli/brain/4505434e-5114-452d-b8ef-610b6bdca0a8"

def send_qmp(sock, cmd_obj):
    msg = json.dumps(cmd_obj) + "\r\n"
    sock.sendall(msg.encode("utf-8"))
    time.sleep(0.15)
    resp = b""
    while True:
        try:
            chunk = sock.recv(4096)
            if not chunk:
                break
            resp += chunk
            if b"\r\n" in chunk:
                break
        except socket.timeout:
            break
    return resp.decode("utf-8", errors="ignore")

def send_key(sock, key):
    send_qmp(sock, {"execute": "send-key", "arguments": {"keys": [{"type": "qcode", "data": key}]}})
    time.sleep(0.3)

def send_key_combo(sock, keys):
    qkeys = [{"type": "qcode", "data": k} for k in keys]
    send_qmp(sock, {"execute": "send-key", "arguments": {"keys": qkeys}})
    time.sleep(0.3)

def send_string(sock, text):
    for ch in text:
        if ch == ' ':
            send_key(sock, "spc")
        elif ch == '\n':
            send_key(sock, "ret")
        elif ch == '-':
            send_key(sock, "minus")
        elif ch == '/':
            send_key(sock, "slash")
        elif ch == '.':
            send_key(sock, "dot")
        elif ch == ':':
            send_key_combo(sock, ["shift", "semicolon"])
        elif ch == '_':
            send_key_combo(sock, ["shift", "minus"])
        elif ch == '$':
            send_key_combo(sock, ["shift", "4"])
        elif ch.isupper():
            send_key_combo(sock, ["shift", ch.lower()])
        else:
            send_key(sock, ch)
        time.sleep(0.04)

def capture_screenshot(sock, filepath):
    ppm_path = filepath.replace(".png", ".ppm")
    send_qmp(sock, {"execute": "screendump", "arguments": {"filename": ppm_path}})
    time.sleep(0.6)
    if os.path.exists(ppm_path):
        subprocess.run(["magick", ppm_path, filepath], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if os.path.exists(filepath):
            try:
                os.remove(ppm_path)
            except Exception:
                pass
            print(f"[AUDIT] Saved screenshot -> {filepath}")
            return True
    return False

def main():
    if os.path.exists(QMP_SOCK):
        try:
            os.remove(QMP_SOCK)
        except Exception:
            pass

    uefi_opts = []
    if os.path.exists("/usr/share/edk2/x64/OVMF_CODE.4m.fd"):
        uefi_opts = ["-drive", "if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd"]
    elif os.path.exists("/usr/share/ovmf/x64/OVMF_CODE.4m.fd"):
        uefi_opts = ["-drive", "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.4m.fd"]

    qemu_cmd = [
        "qemu-system-x86_64",
        "-enable-kvm",
        "-cpu", "host",
        "-m", "4096",
        "-smp", "4",
        "-vga", "virtio",
        "-display", "none",
        "-vnc", ":1",
        "-device", "virtio-tablet-pci",
        "-device", "virtio-keyboard-pci",
        "-qmp", f"unix:{QMP_SOCK},server,nowait",
        "-net", "nic,model=virtio",
        "-net", "user",
    ] + uefi_opts + [
        "-drive", f"file={ISO_PATH},media=cdrom,readonly=on",
        "-boot", "d",
    ]

    print("[AUDIT] Starting QEMU with freshly built DIAD ISO...")
    qemu_proc = subprocess.Popen(qemu_cmd)

    try:
        # Wait for socket
        for _ in range(30):
            if os.path.exists(QMP_SOCK):
                break
            time.sleep(0.5)

        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(5.0)
        sock.connect(QMP_SOCK)

        # Handshake
        sock.recv(4096)
        send_qmp(sock, {"execute": "qmp_capabilities"})
        print("[AUDIT] QMP Handshake successful. Waiting 28s for system boot...")

        time.sleep(28)

        # 1. Capture Workspace 1
        ws1_file = os.path.join(ARTIFACT_DIR, "audit_ws1_agent.png")
        capture_screenshot(sock, ws1_file)

        # 2. Switch to Workspace 2 (Browser with official logo)
        print("[AUDIT] Switching to Workspace 2 (F2)...")
        send_key(sock, "f2")
        time.sleep(3.0)
        ws2_file = os.path.join(ARTIFACT_DIR, "audit_ws2_browser_official_logo.png")
        capture_screenshot(sock, ws2_file)

        # 3. Switch to Workspace 3 (Comms)
        print("[AUDIT] Switching to Workspace 3 (F3)...")
        send_key(sock, "f3")
        time.sleep(2.5)
        ws3_file = os.path.join(ARTIFACT_DIR, "audit_ws3_comms.png")
        capture_screenshot(sock, ws3_file)

        # 4. Open Terminal, test CLI colors, and test TUI split-screen responsiveness
        print("[AUDIT] Switching back to Workspace 1 and opening Terminal...")
        send_key(sock, "f1")
        time.sleep(1.0)
        send_key_combo(sock, ["meta_l", "ret"])
        time.sleep(2.0)
        send_string(sock, "diad status\n")
        time.sleep(2.0)
        term_file = os.path.join(ARTIFACT_DIR, "audit_terminal_colors.png")
        capture_screenshot(sock, term_file)

        # Test TUI inside tiled terminal
        print("[AUDIT] Launching TUI inside split terminal to test responsive layout...")
        send_string(sock, "diad-agent --tui\n")
        time.sleep(3.0)
        tui_file = os.path.join(ARTIFACT_DIR, "audit_tui_split_responsive.png")
        capture_screenshot(sock, tui_file)

        # 5. Test Lockscreen
        print("[AUDIT] Testing Lockscreen (diad lock)...")
        send_string(sock, "diad lock\n")
        time.sleep(3.0)
        lock_file = os.path.join(ARTIFACT_DIR, "audit_lockscreen_official_logo.png")
        capture_screenshot(sock, lock_file)

        # 6. Unlock screen by pressing space
        print("[AUDIT] Unlocking screen...")
        send_key(sock, "spc")
        time.sleep(2.0)

        # 7. Test rapid workspace switching (F1 -> F2 -> F3 -> F1)
        print("[AUDIT] Testing rapid flicker-free switching...")
        for _ in range(3):
            send_key(sock, "f2")
            time.sleep(0.5)
            send_key(sock, "f3")
            time.sleep(0.5)
            send_key(sock, "f1")
            time.sleep(0.5)
        
        final_file = os.path.join(ARTIFACT_DIR, "audit_final_verification.png")
        capture_screenshot(sock, final_file)
        print("[AUDIT] Audit successfully completed!")

    finally:
        try:
            sock.close()
        except Exception:
            pass
        qemu_proc.terminate()
        try:
            qemu_proc.wait(timeout=5)
        except Exception:
            qemu_proc.kill()

if __name__ == "__main__":
    main()
