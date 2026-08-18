#!/usr/bin/env bash
# =============================================================================
# DIAD OS — Virtual Machine Launcher (QEMU + KVM + UEFI)
# =============================================================================
# High-performance local VM with:
#   - UEFI (OVMF) fast boot
#   - KVM hardware acceleration
#   - Tablet absolute mouse (moves freely without cursor trap)
#   - User internet pass-through with SSH forward (port 2222 -> 22)
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
fi

exec qemu-system-x86_64 \
    ${KVM_FLAG} \
    -m 4G \
    -smp 4 \
    -vga virtio \
    -display gtk,grab-on-hover=off,zoom-to-fit=on \
    -device virtio-tablet-pci \
    -device virtio-keyboard-pci \
    -device virtio-net-pci,netdev=net0 \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    -qmp unix:/tmp/qmp.sock,server,nowait \
    "${UEFI_OPTS[@]}" \
    -drive "file=${ISO_PATH},media=cdrom,readonly=on" \
    -boot d
