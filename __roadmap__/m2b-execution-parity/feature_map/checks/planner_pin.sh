#!/bin/sh
# S3.R19: sandbox planner = the single gtpyhop/gtpyhop-core pin of the skill's own scripts/pyproject.toml.
: "${PROVENANCE_JSON:?}"; : "${WORKSPACE_DIR:?}"
norm() { tr 'A-Z_.' 'a-z--'; }
pp="$WORKSPACE_DIR/scripts/pyproject.toml"
if [ -f "$pp" ]; then
  pin=$(grep -oE '"(gtpyhop-core|gtpyhop)[^"]*"' "$pp" | sed -E 's/^"(gtpyhop-core|gtpyhop).*/\1/' | sort -u)
else pin=gtpyhop-core; fi          # reference fallback for skills with no scripts/pyproject.toml
[ "$(printf '%s\n' "$pin" | wc -l | tr -d ' ')" = 1 ] && [ -n "$pin" ] \
  || { echo "FAIL planner_pin: skill pins zero or several planner distributions: $pin"; exit 1; }
got=$(jq -r '.planner.sandbox.distribution | if type == "string" then . else "NOT-A-SINGLE-STRING:" + tojson end' "$PROVENANCE_JSON" | norm)
[ "$got" = "$pin" ] || { echo "FAIL planner_pin: sandbox planner '$got' != skill pin '$pin'"; exit 1; }
echo "OK planner_pin ($pin)"
