#!/usr/bin/env bash
#
# DIAD — Snap layouts (spec 2.6)
# =============================================================================
# Tiles the first N toplevel windows on the focused output using the
# TESTED integer-pixel geometry engine in the agent binary
# (`diad-agent --snap N` — exact tiling, no gaps/overlaps, portrait-aware).
#
# Usage: diad-snap.sh [N]        N = 1..8 (default 2)
#
set -uo pipefail

N="${1:-2}"
case "${N}" in
    ''|*[!0-9]*) N=2 ;;
esac
(( N >= 1 && N <= 8 )) || N=2

if ! command -v diad-agent >/dev/null 2>&1 || ! command -v swaymsg >/dev/null 2>&1; then
    echo "diad-snap: needs diad-agent + swaymsg" >&2
    exit 1
fi

# ---- Geometry from the tested engine (one "x y w h" line per tile) ----
mapfile -t RECTS < <(diad-agent --snap "${N}")
if [[ ${#RECTS[@]} -eq 0 ]]; then
    echo "diad-snap: no geometry produced" >&2
    exit 1
fi

# ---- Toplevel views (leaves carrying an app_id/window), focus order ----
mapfile -t WINDOWS < <(swaymsg -t get_tree 2>/dev/null | jq -r '
    .. | objects | select(.type? == "con")
    | select((.app_id? != null) or (.window? != null))
    | select(.nodes? == [] or (.nodes? | length) == 0)
    | .id')

if [[ ${#WINDOWS[@]} -eq 0 ]]; then
    echo "diad-snap: no windows to snap" >&2
    exit 1
fi

# ---- Apply: tile i gets rect i (windows beyond N are left alone) ------
count=$(( ${#WINDOWS[@]} < N ? ${#WINDOWS[@]} : N ))
for (( i = 0; i < count; i++ )); do
    read -r x y w h <<< "${RECTS[$i]}"
    id="${WINDOWS[$i]}"
    swaymsg "[con_id=${id}] move position ${x} ${y}" >/dev/null 2>&1
    swaymsg "[con_id=${id}] resize set ${w} ${h}" >/dev/null 2>&1
done

echo "diad-snap: tiled ${count} window(s)."
exit 0
