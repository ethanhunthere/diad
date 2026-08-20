# B — ISO Artifact Audit (diados-2026.08.18-x86_64.iso)

Read-only verification. Built 2026-08-18; invariant: ISO 800–900 MB (target ~820 MB), uncompressed pkg tree < 1.6 GB (QWEN_AUDIT_PROMPT.md §02, Invariant 2).

## Verdict table

| Check | Evidence | Verdict |
|---|---|---|
| ISO 800–900 MB invariant | 1,704,062,976 B = **1704.06 MB / 1625.12 MiB** | **FAIL** (+804 MB over ceiling, 1.89×) |
| 900 MB hard ceiling | 1704.06 MB > 900 MB | **FAIL** |
| Uncompressed pkg tree < 1.6 GB | unsquashfs: 1,429,214,850 B = 1.43 GB | PASS (hollow win, see below) |
| UUID consistency | blkid `2026-08-18-00-00-00-00` == `work/iso/boot/2026-08-18-00-00-00-00.uuid` | PASS |
| kernel/initrd match staging↔ISO | sha256 identical after xorriso extract (`fd8bbbd3…`, `f7e80b43…`) | PASS |
| airootfs.sfs match staging↔ISO | sha512 `3a0e91fe…` identical to `airootfs.sha512` | PASS |
| Compression config sane | squashfs zstd level 19, 256K blocks (profiledef.sh) | PASS — but ratio ≈ 1.0 |

Key pathology: sfs (1449.68 MB) is **larger than** its uncompressed tree (1429.2 MB). Content is
incompressible (firmware blobs, Chromium's pre-compressed resources), so zstd-19 buys nothing.

## 1. Sizes & trend (out/) — every ISO ever built violates the ceiling

| ISO | Bytes | MB | vs 900 |
|---|---|---|---|
| 2026.01.01 | 1,127,319,552 | 1127.32 | FAIL |
| 2026.08.12 | 3,606,499,328 | 3606.50 | FAIL (debug-bloat outlier) |
| 2026.08.14 | 3,627,913,216 | 3627.91 | FAIL |
| 2026.08.15 | 1,070,090,240 | 1070.09 | FAIL (closest so far) |
| 2026.08.16 | 1,372,413,952 | 1372.41 | FAIL |
| 2026.08.17 | 1,703,960,576 | 1703.96 | FAIL |
| **2026.08.18** | **1,704,062,976** | **1704.06** | **FAIL** (+102 KB vs 08.17) |

## 2. Staging tree breakdown (work/iso/)

- `diados/x86_64/airootfs.sfs` — **squashfs** (.sfs), 1,449,684,992 B (1382.5 MiB) = **85.1% of ISO**; config: `airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '19' '-b' '256K')`
- `diados/boot/x86_64/vmlinuz-linux-lts` — 16,437,760 B (15.7 MiB)
- `diados/boot/x86_64/initramfs-linux-lts.img` — **200,675,363 B (191.4 MiB)** — anomalously large; mkinitcpio.conf.d/archiso.conf: HOOKS include kms+microcode, `COMPRESSION="zstd"` at **-1** (speed over size, P3-S2.3)
- `EFI/BOOT/BOOTx64.EFI` 7,712,768 B + `BOOTIA32.EFI` 6,238,208 B = 13.95 MB total
- `boot/grub` 12 KB; `diados/pkglist.x86_64.txt`, grubenv, version (KB-scale)
- Math closes: 1449.7 + 200.7 + 16.4 + 14.0 + grub/ISO overhead ≈ 1704 MB

Inside the sfs (measured via unsquashfs): `/usr/lib/firmware` **414 MB** (intel 125M, nvidia 109M,
mediatek 39M, amdgpu 28M, ath11k 24M, ath12k 17M); `/usr/lib/chromium` **397 MB**; dev toolchain
~380 MB (`usr/lib/gcc` 87M, `usr/include` 132M, gdb 31M, cmake suite ~60M, rustup 12M, chromedriver
21M, gettext 13M); `usr/lib/dri` 41M. `/usr/local/bin/brave` is declared in file_permissions but
**absent** from the image (stale entry).

## 3. Package list — 461 packages

Heaviest contributors present: **chromium 151** (~400 MB in tree), **linux-firmware + all 10 split
packages** (~414 MB, incompressible — dominates the zstd image), llvm-libs 22 (mesa/rust deps),
full **gcc/cmake/rustup/gdb toolchain**, mesa + vulkan, pipewire/wireplumber stack (small, fine),
grub 2.14 (fine). No webkit2gtk/epiphany/ffmpeg — chromium is the browser. Firmware + toolchain are
the removable mass, not the audio/GUI stack.

## 4. Consistency — PASS

blkid UUID matches the .uuid filename; vmlinuz, initramfs, and airootfs.sfs extracted from the ISO
hash-identical to work/iso staging. ISO is a faithful image of this staging tree.

## 5. Closing the gap — need ≥ 804 MB cut (884 MB to hit ~820 target)

Content is ~1:1 incompressible, so cuts are near-literal:

1. **Drop dev toolchain from live image** (~380 MB): gcc, headers, gdb, cmake, rustup, chromedriver.
   Audit §Q10 explicitly demands dev ergonomics "without bloating the base ISO". Biggest single lever.
2. **Firmware trim** (~250–300 MB): drop nvidia/mediatek/cirrus/dpaa2/ti/broadcom splits; keep
   intel+amdgpu+atheros+realtek. Incompressible → 1:1 savings.
3. **Initramfs recompression** (~120–140 MB): zstd -1 → zstd -19 (or xz -9); 200.7 MB → ~70 MB.
4. Minor: locales/docs/man sweep, chromedriver 21M, stale brave permission entry.

1+2+3 ≈ 750–820 MB → lands ~885–955 MB; hitting the band requires all three plus minor sweeps.
Compression alone cannot help (ratio already ≈ 1.0) — package removal is the only real path.
