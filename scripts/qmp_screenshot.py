import socket
import json
import sys
import time

def qmp_cmd(sock, execute, arguments=None):
    req = {"execute": execute}
    if arguments:
        req["arguments"] = arguments
    sock.sendall(json.dumps(req).encode('utf-8') + b'\r\n')
    
    # Read response
    while True:
        line = sock.makefile('rb').readline()
        if not line:
            return None
        data = json.loads(line.decode('utf-8'))
        if "return" in data or "error" in data:
            return data

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 qmp_screenshot.py <output.ppm> [keys to send]")
        sys.exit(1)

    out_file = sys.argv[1]
    keys = sys.argv[2:] if len(sys.argv) > 2 else []

    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    import os
    sock_path = "/tmp/qmp.sock" if os.path.exists("/tmp/qmp.sock") else "/tmp/qmp-diad.sock"
    s.connect(sock_path)
    
    # Read greeting
    greeting = s.recv(1024)
    # Negotiate capabilities
    qmp_cmd(s, "qmp_capabilities")

    for k in keys:
        print(f"Sending key: {k}")
        # send key
        qmp_cmd(s, "send-key", {"keys": [{"type": "qcode", "data": k}]})
        time.sleep(0.5)

    print(f"Capturing screendump to {out_file}...")
    res = qmp_cmd(s, "screendump", {"filename": out_file})
    print("Result:", res)
    s.close()

if __name__ == "__main__":
    main()
