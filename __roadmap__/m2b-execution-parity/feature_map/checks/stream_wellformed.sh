#!/bin/sh
# X (usage, tool-call counts, stream archive, denial record): stream-json has an init event, assistant events and a final result with usage and a permission_denials list.
: "${STREAM_JSONL:?}"
jq -nR '[inputs | fromjson? | select(type == "object")]
  | (map(select(.type == "system" and .subtype == "init")) | length >= 1)
    and (map(select(.type == "assistant")) | length >= 1)
    and ((map(select(.type == "result")) | last) as $r
         | $r != null and ($r.usage | type == "object") and ($r.usage | has("input_tokens") and has("output_tokens"))
           and ($r.permission_denials | type == "array"))' "$STREAM_JSONL" | grep -qx true \
  || { echo "FAIL stream_wellformed: init/assistant/result(usage, permission_denials) incomplete in $STREAM_JSONL"; exit 1; }
echo "OK stream_wellformed"
