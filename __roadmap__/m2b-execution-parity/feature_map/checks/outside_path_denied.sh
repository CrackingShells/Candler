#!/bin/sh
# S3.R14, X (hook rule): the agent cannot reach the host outside its work dir.
# v5 enforced this with a PreToolUse hook that refused outside paths. Under container isolation (Eliott, 2026-11-06:
# keep the feature, not the mechanism; A55, F11) the boundary is the mount table: an outside path the agent names
# resolves inside its own image, which is allowed. The check therefore fails when the agent container's mounts reach
# the host anywhere beyond: the work dir (rw bind), HOME (rw bind), read-only input binds under the work dir, and the
# anonymous env volume at /opt/env (A13). Kernel facts (EROFS root, host sentinel absent) stay with B06 and host_isolation.
: "${INSPECT_JSON:?INSPECT_JSON unset: docker inspect of the agent container}"
c='(if type == "array" then .[0] else . end)'
cwd=$(jq -r "$c"' | .Config.WorkingDir' "$INSPECT_JSON")
home=$(jq -r "$c"' | [.Config.Env[]? | select(startswith("HOME=")) | sub("^HOME="; "")] | first // ""' "$INSPECT_JSON")
[ -n "$cwd" ] && [ "$cwd" != null ] && [ -n "$home" ] || { echo "FAIL outside_path_denied: inspect lacks WorkingDir or HOME"; exit 1; }
bad=$(jq -r --arg cwd "$cwd" --arg home "$home" "$c"' | .Mounts[]?
  | select(
      ((.Type == "bind") and ((.Destination == $cwd or .Destination == $home) and .RW == true)) or
      ((.Type == "bind") and (.RW == false) and (.Destination | startswith($cwd + "/"))) or
      ((.Type == "volume") and (.Destination == "/opt/env") and ((.Name // "") | test("^[0-9a-f]{64}$")))
      | not)
  | "\(.Type) \(.Source) -> \(.Destination) rw=\(.RW)"' "$INSPECT_JSON")
[ -z "$bad" ] || { echo "FAIL outside_path_denied: undeclared mount(s): $bad"; exit 1; }
n=0
if [ -n "${STREAM_JSONL:-}" ] && [ -f "$STREAM_JSONL" ]; then
  n=$(jq -nR --arg cwd "$cwd" '[inputs | fromjson? | select(type == "object" and .type == "assistant") | .message.content[]?
    | select(.type == "tool_use" and (.name | IN("Read","Write","Edit"))) | .input.file_path // ""
    | select(startswith("/") and (startswith($cwd + "/") | not) and . != $cwd)] | length' "$STREAM_JSONL")
fi
echo "OK outside_path_denied (mounts within the declared set; $n Read/Write/Edit call(s) named an in-image path outside $cwd)"
