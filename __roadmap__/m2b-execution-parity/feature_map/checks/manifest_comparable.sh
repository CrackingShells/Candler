#!/bin/sh
# X27: comparability data is canonical and every compared value has a recorded source (I12).
# Each list rises strictly by its R01 key, so it is sorted and holds no duplicate: values by path,
# snapshots and inputs by kind then digest, environments by plugin then lock digest.
: "${RUN_DIR:?}"; m="$RUN_DIR/manifest.json"
jq -e '
 def ascending(key): [.[] | key] as $k | all(range(1; $k | length); $k[. - 1] < $k[.]);
 .comparable as $c
 | ($c.values // [] | ascending(.path))
 and ($c.snapshots | ascending([.kind, .digest]))
 and ($c.inputs | ascending([.kind, .digest]))
 and ($c.environments | ascending([.plugin, .lock_digest]))
 and ($c.values // [] | map(.path) | all(. as $p | $p | IN($m.sources[]?.path)))
' --argjson m "$(jq . "$m")" "$m" >/dev/null 2>&1 \
 || { echo "FAIL manifest_comparable: a list does not rise strictly by its key or a value has no source"; exit 1; }
echo "OK manifest_comparable"
