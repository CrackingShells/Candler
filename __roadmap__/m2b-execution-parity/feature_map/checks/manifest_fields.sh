#!/bin/sh
# X28: the run manifest carries the per-run record R01 names; integrity and secrets are well formed.
: "${RUN_DIR:?}"; m="$RUN_DIR/manifest.json"
[ -f "$m" ] || { echo "FAIL manifest_fields: no manifest.json"; exit 1; }
jq -e '
 (["run","experiment","factors","comparable","sources","secrets","measured","integrity"] | all(. as $k | $m | has($k))) ' --argjson m "$(cat "$m")" -n >/dev/null 2>&1 \
 || { echo "FAIL manifest_fields: missing top-level key"; exit 1; }
jq -e '
 (.comparable | has("harness") and has("treatment") and has("snapshots") and has("inputs") and has("environments"))
 and (.comparable.harness | has("name") and has("version"))
 and ((.integrity.kind=="valid") or (.integrity.kind=="invalid" and (.integrity.reason|type=="string" and length>0)))
 and (.secrets | all(.[]; has("name") and has("present") and ((keys - ["name","present"])|length==0)))
' "$m" >/dev/null || { echo "FAIL manifest_fields: comparable/integrity/secrets malformed"; exit 1; }
echo "OK manifest_fields"
