#!/bin/sh
# S4, S3.R14: container is the boundary: unprivileged, no host pid/ipc, no extra caps, no socket or host-root/credential mounts.
: "${INSPECT_JSON:?}"
v=$(jq -r '.[0] // . | [
  (if .HostConfig.Privileged then "privileged" else empty end),
  (if .HostConfig.PidMode == "host" then "host pid" else empty end),
  (if .HostConfig.IpcMode == "host" then "host ipc" else empty end),
  (if (.HostConfig.CapAdd // []) | length > 0 then "CapAdd" else empty end),
  (if ((.HostConfig.SecurityOpt // []) | any(test("unconfined"))) then "unconfined security-opt" else empty end),
  (.Mounts // [] | .[] | select(.Type == "bind") | select(.Source == "/" or (.Source | test("docker\\.sock|/\\.ssh|/\\.aws|/\\.docker|/\\.gnupg|^/(Users|home)/[^/]+/?$"))) | "bad mount " + .Source)
] | .[]' "$INSPECT_JSON") || { echo "FAIL host_isolation: cannot parse INSPECT_JSON"; exit 1; }
[ -z "$v" ] || { echo "FAIL host_isolation: $(echo "$v" | head -1)"; exit 1; }
echo "OK host_isolation"
