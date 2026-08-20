#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Virtual Machine Launcher (QEMU + KVM + UEFI)
# =============================================================================
# High-performance local VM with:
#   - UEFI (OVMF) fast boot
#   - KVM hardware acceleration
#   - Tablet absolute mouse (moves freely without cursor trap)
#   - Dynamic SSH port forwarding (auto-finds free port from 2222+)
#   - QMP monitor socket at /tmp/qmp.sock
#   - Release key: Ctrl + Alt + G
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

ISO_PATH="${1:-$(ls -t "${ROOT_DIR}"/out/*.iso 2>/dev/null | head -n 1 || true)}"

if [[ -z "${ISO_PATH}" || ! -f "${ISO_PATH}" ]]; then
    echo "❌ No ISO found in ${ROOT_DIR}/out/."
    echo "💡 Run 'make iso' first, or provide an ISO path: ./scripts/run-vm.sh path/to/diados.iso"
    exit 1
fi

echo "🚀 Starting DIAD OS in QEMU VM..."
echo "📦 ISO: ${ISO_PATH}"

KVM_FLAG=""
if [[ -e /dev/kvm && -w /dev/kvm ]]; then
    KVM_FLAG="-enable-kvm -cpu host"
    echo "⚡ KVM Hardware Acceleration: ENABLED"
else
    echo "⚠️  KVM not available — running in software emulation"
fi

UEFI_OPTS=()
if [[ -f /usr/share/edk2/x64/OVMF_CODE.4m.fd ]]; then
    UEFI_OPTS=(-drive "if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd")
    echo "⚡ UEFI (OVMF) Boot: ENABLED"
elif [[ -f /usr/share/ovmf/x64/OVMF_CODE.4m.fd ]]; then
    UEFI_OPTS=(-drive "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.4m.fd")
    echo "⚡ UEFI (OVMF) Boot: ENABLED"
fi

# Clean up any stale QMP socket
rm -f /tmp/qmp.sock /tmp/qmp-diad.sock

# Find first available host port for SSH forwarding (starts at 2222)
SSH_PORT=2222
while ss -tulpn 2>/dev/null | grep -q ":${SSH_PORT} "; do
    SSH_PORT=$((SSH_PORT + 1))
done
echo "🔌 Host SSH forward: localhost:${SSH_PORT} -> guest:22"

DISPLAY_OPTS=("-display" "gtk,grab-on-hover=off,zoom-to-fit=on")
if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
    DISPLAY_OPTS=("-display" "none" "-vnc" ":1")
    echo "🖥️ Headless mode: VNC available at :1 (port 5901)"
fi

exec qemu-system-x86_64 \
    ${KVM_FLAG} \
    -m 4G \
    -smp 4 \
    -vga virtio \
    "${DISPLAY_OPTS[@]}" \
    -device virtio-tablet-pci \
    -device virtio-keyboard-pci \
    -device virtio-net-pci,netdev=net0 \
    -netdev "user,id=net0,hostfwd=tcp::${SSH_PORT}-:22" \
    -qmp unix:/tmp/qmp.sock,server,nowait \
    "${UEFI_OPTS[@]}" \
    -drive "file=${ISO_PATH},media=cdrom,readonly=on" \
    -boot d
