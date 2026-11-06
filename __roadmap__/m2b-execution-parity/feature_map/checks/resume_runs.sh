#!/bin/sh
# X22/X23: sealed runs are untouched; attempts per (cell, replicate) are numbered 1..n with no gap and no duplicate
# (nothing deleted); an unsealed attempt that has a successor is recorded invalid(interrupted).
# Assumes manifest.run is an object with cell (digest), replicate and attempt.
: "${STORE_DIR:?}"
tmp=$(mktemp) || exit 1; trap 'rm -f "$tmp"' EXIT
for r in "$STORE_DIR"/runs/*; do
  [ -d "$r" ] || continue
  t=$(jq -r '[(.run.cell // .run.cell_digest // ""), (.run.replicate // ""), (.run.attempt // "")] | @tsv' "$r/manifest.json" 2>/dev/null)
  [ "$(printf "%s" "$t" | awk -F"\t" "{n=0; for(i=1;i<=3;i++) if(\$i!=\"\") n++; print n}")" = 3 ] || { echo "FAIL resume_runs: $r manifest.run lacks cell/replicate/attempt"; exit 1; }
  if [ -f "$r/run.done" ]; then
    n=$(find "$r" -type f ! -name run.done -newer "$r/run.done" | wc -l | tr -d ' ')
    [ "$n" -eq 0 ] || { echo "FAIL resume_runs: sealed $r modified after run.done"; exit 1; }
    s=sealed
  else s=open; fi
  ik=$(jq -r '.integrity.kind + ":" + (.integrity.reason // "")' "$r/manifest.json")
  printf '%s\t%s\t%s\n' "$t" "$s" "$ik" >> "$tmp"
done
[ -s "$tmp" ] || { echo "FAIL resume_runs: no runs"; exit 1; }
cut -f1,2 "$tmp" | sort -u | while IFS='	' read -r c rep; do
  a=$(awk -F'\t' -v c="$c" -v r="$rep" '$1==c && $2==r {print $3}' "$tmp" | sort -n | tr '\n' ' ')
  exp=$(awk -v n="$(echo "$a" | wc -w)" 'BEGIN{for(i=1;i<=n;i++) printf "%d ", i}')
  [ "$a" = "$exp" ] || { echo "FAIL resume_runs: attempts for $c/$rep are [$a], expected [$exp]"; exit 1; }
done || exit 1
mx=$(awk -F'\t' '{k=$1 FS $2; if ($3>m[k]) m[k]=$3} END{for(k in m) print k "\t" m[k]}' "$tmp")
awk -F'\t' '$4=="open" && $5!="invalid:interrupted" {print $1" "$2" attempt "$3}' "$tmp" | while read -r l; do
  c=$(echo "$l" | cut -d' ' -f1); rep=$(echo "$l" | cut -d' ' -f2); at=$(echo "$l" | awk '{print $4}')
  top=$(printf '%s\n' "$mx" | awk -F'\t' -v c="$c" -v r="$rep" '$1==c && $2==r {print $3}')
  [ "$at" -lt "$top" ] && { echo "FAIL resume_runs: unsealed attempt $at of $c/$rep has a successor but is not invalid(interrupted)"; exit 1; }
done || exit 1
echo "OK resume_runs"
