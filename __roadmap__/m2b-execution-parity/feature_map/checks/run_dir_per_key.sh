#!/bin/sh
# X16: every run has its own run dir; no two runs share one, none is a link, each has a manifest with a distinct run key.
: "${STORE_DIR:?}"
[ -f "$STORE_DIR/store.json" ] || { echo "FAIL run_dir_per_key: no store.json marker"; exit 1; }
[ -d "$STORE_DIR/runs" ] || { echo "FAIL run_dir_per_key: no runs/"; exit 1; }
n=0; keys=""
for r in "$STORE_DIR"/runs/*; do
  [ -e "$r" ] || continue
  [ -L "$r" ] && { echo "FAIL run_dir_per_key: $r is a symlink"; exit 1; }
  [ -d "$r" ] || { echo "FAIL run_dir_per_key: $r is not a dir"; exit 1; }
  [ -f "$r/manifest.json" ] || { echo "FAIL run_dir_per_key: $r has no manifest.json"; exit 1; }
  k=$(jq -c '.run' "$r/manifest.json") || { echo "FAIL run_dir_per_key: unreadable manifest in $r"; exit 1; }
  [ "$k" != null ] || { echo "FAIL run_dir_per_key: manifest in $r has no run"; exit 1; }
  keys="$keys
$k"; n=$((n+1))
done
[ "$n" -gt 0 ] || { echo "FAIL run_dir_per_key: no runs"; exit 1; }
u=$(printf '%s\n' "$keys" | sed '/^$/d' | sort -u | wc -l | tr -d ' ')
[ "$u" -eq "$n" ] || { echo "FAIL run_dir_per_key: $n run dirs but $u distinct run keys"; exit 1; }
echo "OK run_dir_per_key ($n runs)"
