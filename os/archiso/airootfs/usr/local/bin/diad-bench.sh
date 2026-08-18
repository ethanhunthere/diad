#!/usr/bin/env bash
#
# DIAD — performance bench & regression guard (spec 5.2 / 5.3)
# =============================================================================
# Prints a full system + daemon report. Modes:
#
#   diad-bench.sh           one-shot human-readable report
#   diad-bench.sh --watch   continuous 1 s sampling (Ctrl+C to stop)
#   diad-bench.sh --json    machine-readable single sample
#   diad-bench.sh --check   compare against /etc/diad/perf-baseline.json:
#                           creates the baseline on first run, flags any
#                           metric worse than 15% afterwards (exit 1)
#
set -uo pipefail

BASELINE_FILE="/etc/diad/perf-baseline.json"
DAEMON_URL="http://127.0.0.1:7860"

# ---------------------------------------------------------------- helpers --

proc_rss_kb() { # rss of first process matching a pattern
    local pid
    pid="$(pgrep -f "$1" 2>/dev/null | head -1)" || return 1
    [[ -n "${pid}" ]] || return 1
    awk '/^VmRSS:/ {print $2}' "/proc/${pid}/status" 2>/dev/null
}

collect_json() {
    local daemon_pid daemon_rss_kb idle_stage tier
    daemon_pid="$(pgrep -f 'diad-agent --daemon' 2>/dev/null | head -1)"
    daemon_rss_kb=0
    idle_stage="n/a"
    if [[ -n "${daemon_pid}" ]]; then
        daemon_rss_kb="$(awk '/^VmRSS:/ {print $2}' "/proc/${daemon_pid}/status" 2>/dev/null || echo 0)"
        idle_stage="$(curl -fs --max-time 2 "${DAEMON_URL}/api/v1/status" 2>/dev/null | jq -r '.idle_stage // "n/a"' 2>/dev/null || echo "n/a")"
    fi
    tier="$(cat /etc/diad/perf-tier 2>/dev/null | tr -d '[:space:]')"
    [[ -n "${tier}" ]] || tier="n/a"

    local boot_s mem_free_mb zram_used governor gpu storage
    boot_s="$(systemd-analyze time 2>/dev/null | grep -oP 'Startup finished.*?=\s*\K[0-9.]+' | tail -1)"
    [[ -n "${boot_s}" ]] || boot_s="0"
    mem_free_mb="$(free -m | awk '/^Mem:/ {print $4}')"
    zram_used="$(zramctl --bytes 2>/dev/null | awk 'NR==2 {print $4}')"
    [[ -n "${zram_used}" ]] || zram_used="0"
    governor="$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo n/a)"
    gpu="$(lspci -nn 2>/dev/null | grep -E 'VGA|3D' | head -1 | sed 's/.*: //; s/ \[.*//' | cut -c1-60)"
    [[ -n "${gpu}" ]] || gpu="n/a"
    storage="unknown"
    for disk in /sys/block/*; do
        case "$(basename "${disk}")" in
            nvme*) storage="nvme"; break ;;
            loop*|ram*|zram*|dm-*|sr*|md*) continue ;;
            *)
                if [[ "$(cat "${disk}/queue/rotational" 2>/dev/null)" == "1" ]]; then
                    storage="hdd"
                else
                    storage="ssd"
                fi
                ;;
        esac
    done

    jq -n \
        --argjson boot_s "${boot_s:-0}" \
        --argjson daemon_rss_kb "${daemon_rss_kb:-0}" \
        --arg idle_stage "${idle_stage}" \
        --arg tier "${tier}" \
        --argjson mem_free_mb "${mem_free_mb:-0}" \
        --argjson brave_rss_kb "$(proc_rss_kb brave || echo 0)" \
        --argjson sway_rss_kb "$(proc_rss_kb '^sway$' || echo 0)" \
        --argjson waybar_rss_kb "$(proc_rss_kb waybar || echo 0)" \
        --argjson zram_used "${zram_used:-0}" \
        --arg governor "${governor}" \
        --arg gpu "${gpu}" \
        --arg storage "${storage}" \
        '{
            boot_seconds: $boot_s,
            daemon_rss_mb: ($daemon_rss_kb / 1024 | round),
            daemon_mode: $idle_stage,
            perf_tier: $tier,
            mem_free_mb: $mem_free_mb,
            brave_rss_mb: ($brave_rss_kb / 1024 | round),
            sway_rss_mb: ($sway_rss_kb / 1024 | round),
            waybar_rss_mb: ($waybar_rss_kb / 1024 | round),
            zram_used_bytes: $zram_used,
            governor: $governor,
            gpu: $gpu,
            storage: $storage
        }'
}

print_report() {
    local j="$1"
    echo "================ DIAD bench ================"
    printf '%-22s %s\n' "boot time:" "$(jq -r '.boot_seconds' <<<"${j}") s"
    printf '%-22s %s\n' "perf tier:" "$(jq -r '.perf_tier' <<<"${j}")"
    printf '%-22s %s\n' "daemon mode:" "$(jq -r '.daemon_mode' <<<"${j}")"
    printf '%-22s %s\n' "daemon RSS:" "$(jq -r '.daemon_rss_mb' <<<"${j}") MB"
    printf '%-22s %s\n' "free memory:" "$(jq -r '.mem_free_mb' <<<"${j}") MB"
    printf '%-22s %s\n' "brave RSS:" "$(jq -r '.brave_rss_mb' <<<"${j}") MB"
    printf '%-22s %s\n' "sway RSS:" "$(jq -r '.sway_rss_mb' <<<"${j}") MB"
    printf '%-22s %s\n' "waybar RSS:" "$(jq -r '.waybar_rss_mb' <<<"${j}") MB"
    printf '%-22s %s\n' "zram in use:" "$(jq -r '.zram_used_bytes' <<<"${j}") B"
    printf '%-22s %s\n' "cpu governor:" "$(jq -r '.governor' <<<"${j}")"
    printf '%-22s %s\n' "gpu:" "$(jq -r '.gpu' <<<"${j}")"
    printf '%-22s %s\n' "storage:" "$(jq -r '.storage' <<<"${j}")"
    echo "---------------------------------------------"
    if command -v systemd-analyze >/dev/null 2>&1; then
        echo "top-5 blame:"
        systemd-analyze blame 2>/dev/null | head -5 | sed 's/^/  /'
    fi
}

# ------------------------------------------------------------- regression --

check_regression() {
    local current="$1"
    if [[ ! -f "${BASELINE_FILE}" ]]; then
        if [[ -w "$(dirname "${BASELINE_FILE}")" ]] || [[ $(id -u) -eq 0 ]]; then
            printf '%s\n' "${current}" > "${BASELINE_FILE}"
            echo "bench: baseline created at ${BASELINE_FILE}"
        else
            echo "bench: no baseline and no permission to write ${BASELINE_FILE}" >&2
            echo "${current}"
        fi
        return 0
    fi

    local baseline fail=0
    baseline="$(cat "${BASELINE_FILE}")"
    echo "bench: comparing against ${BASELINE_FILE} (15% tolerance)"

    # Lower-is-better metrics: current must not exceed baseline * 1.15.
    for metric in boot_seconds daemon_rss_mb; do
        local b c
        b="$(jq -r ".${metric}" <<<"${baseline}")"
        c="$(jq -r ".${metric}" <<<"${current}")"
        if [[ "${b}" == "null" || "${c}" == "null" || "${b}" == "0" ]]; then
            continue
        fi
        if awk "BEGIN {exit !(${c} > ${b} * 1.15)}"; then
            echo "  REGRESSION ${metric}: ${c} vs baseline ${b}"
            fail=1
        else
            echo "  ok ${metric}: ${c} (baseline ${b})"
        fi
    done

    # Higher-is-better metrics: free memory must not drop below 85%.
    local b c
    b="$(jq -r '.mem_free_mb' <<<"${baseline}")"
    c="$(jq -r '.mem_free_mb' <<<"${current}")"
    if [[ "${b}" != "null" && "${c}" != "null" && "${b}" != "0" ]]; then
        if awk "BEGIN {exit !(${c} < ${b} * 0.85)}"; then
            echo "  REGRESSION mem_free_mb: ${c} vs baseline ${b}"
            fail=1
        else
            echo "  ok mem_free_mb: ${c} (baseline ${b})"
        fi
    fi

    return "${fail}"
}

# ------------------------------------------------------------------- main --

case "${1:-}" in
    --json)
        collect_json
        ;;
    --watch)
        trap 'echo; exit 0' INT TERM
        while true; do
            clear 2>/dev/null || printf '\n\n\n\n\n\n\n\n\n\n\n\n\n'
            print_report "$(collect_json)"
            echo "sampling every 1 s — Ctrl+C to stop"
            sleep 1
        done
        ;;
    --check)
        current="$(collect_json)"
        if check_regression "${current}"; then
            echo "bench: PASS"
        else
            echo "bench: FAIL — regression detected"
            exit 1
        fi
        ;;
    *)
        print_report "$(collect_json)"
        ;;
esac
