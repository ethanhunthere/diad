#!/usr/bin/env bash
#
# DIAD — Boot splash
# =============================================================================
# Shown in the DIAD Agent's terminal before the agent starts: ASCII logo,
# "Loading DIAD OS..." progress bar (~3 s), then the terminal clears and the
# agent launches. A splash never delays boot — it's purely cosmetic.
#
set -uo pipefail

clear

cat <<'EOF'

   ██████╗ ██╗     █████╗ ██████╗
   ██╔══██╗██║    ██╔══██╗██╔══██╗
   ██║  ██║██║    ███████║██║  ██║
   ██║  ██║██║    ██╔══██║██║  ██║
   ██████╔╝██║    ██║  ██║██████╔╝
   ╚═════╝ ╚═╝    ╚═╝  ╚═╝╚═════╝

EOF

printf "   Loading DIAD OS  ["
for _ in $(seq 1 20); do
    printf "#"
    sleep 0.15
done
printf "]\n"

sleep 0.5
clear

# ---- Hand off to the agent (graceful if the binary is missing) ----
if command -v diad-agent >/dev/null 2>&1; then
    exec diad-agent
fi

cat <<'EOF'

   diad-agent was not found in PATH.
   It should live at /usr/local/bin/diad-agent.
   Rebuild the ISO with `make iso` to include it.

EOF
read -r -s -p "   Press Enter to close this window..."
exit 1
