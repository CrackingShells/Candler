#!/bin/sh
# X27: comparability data is canonical (lists sorted, no duplicate) and every compared value has a recorded source (I12).
: "${RUN_DIR:?}"; m="$RUN_DIR/manifest.json"
jq -e '
 def canon: . == (sort) and . == (unique);
 (.comparable.snapshots|canon) and (.comparable.inputs|canon) and (.comparable.environments|canon)
 and ((.comparable.values // []) | map(.path) | canon)
 and ((.comparable.values // []) | map(.path) | all(. as $p | $p | IN($m.sources[]?.path)))
' --argjson m "$(jq . "$m")" "$m" >/dev/null 2>&1 \
 || { echo "FAIL manifest_comparable: lists not canonical or a value has no source"; exit 1; }
echo "OK manifest_comparable"
