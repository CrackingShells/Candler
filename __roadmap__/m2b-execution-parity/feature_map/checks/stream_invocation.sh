#!/bin/sh
# X (flags): init event shows permissionMode=bypassPermissions, a model, and cwd = the work dir the agent container starts in.
: "${STREAM_JSONL:?}"; : "${EXEC:?}"
i=$(jq -nR '[inputs | fromjson? | select(type == "object" and .type == "system" and .subtype == "init")] | first // empty' "$STREAM_JSONL")
[ -n "$i" ] || { echo "FAIL stream_invocation: no init event"; exit 1; }
[ "$(printf '%s' "$i" | jq -r .permissionMode)" = bypassPermissions ] || { echo "FAIL stream_invocation: permissionMode != bypassPermissions"; exit 1; }
[ -n "$(printf '%s' "$i" | jq -r '.model // empty')" ] || { echo "FAIL stream_invocation: no model"; exit 1; }
c=$(printf '%s' "$i" | jq -r .cwd); w=$($EXEC pwd -P)
[ "$c" = "$w" ] || { echo "FAIL stream_invocation: init cwd '$c' != container work dir '$w'"; exit 1; }
echo "OK stream_invocation"
