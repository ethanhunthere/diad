#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Virtual Machine Launcher (QEMU + KVM)
# =============================================================================
# Seamless, non-blocking desktop VM with:
#   - Absolute tablet mouse (no cursor trapping or keyboard locking)
#   - Universal VGA display (instant rendering under any Wayland host)
#   - User network pass-through (internet connectivity)
#   - Hotkey to release input if ever needed: Ctrl + Alt + G
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
echo "💡 Note: Mouse moves freely in/out of the window. If input is ever grabbed, press Ctrl+Alt+G to release."

KVM_FLAG=""
if [[ -e /dev/kvm && -w /dev/kvm ]]; then
    KVM_FLAG="-enable-kvm -cpu host"
    echo "⚡ KVM Hardware Acceleration: ENABLED"
else
    echo "⚠️  KVM not available — running in software emulation"
fi

exec qemu-system-x86_64 \
    ${KVM_FLAG} \
    -m 4G \
    -smp 4 \
    -vga std \
    -display gtk,grab-on-hover=off,zoom-to-fit=on \
    -device virtio-tablet-pci \
    -device virtio-keyboard-pci \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0 \
    -cdrom "${ISO_PATH}" \
    -boot d
