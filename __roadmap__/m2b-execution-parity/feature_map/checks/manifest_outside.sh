#!/bin/sh
# S3.R16: no input-hash manifest ({relpath, sha256}) is reachable by the agent, in its container or in its work dir.
: "${EXEC:?}"; : "${WORKSPACE_DIR:?}"
hit=$(grep -rlE --include='*.json' '"relpath"' "$WORKSPACE_DIR" 2>/dev/null | head -1)
[ -z "$hit" ] || { echo "FAIL manifest_outside: $hit lives in the work dir"; exit 1; }
hit=$($EXEC sh -c "grep -rlE --include='*.json' --exclude-dir=proc --exclude-dir=sys --exclude-dir=dev --exclude-dir=usr --exclude-dir=lib '\"relpath\"' / 2>/dev/null | head -1; find / -name '*input_manifest*' -not -path '/proc/*' 2>/dev/null | head -1")
[ -z "$hit" ] || { echo "FAIL manifest_outside: $hit is visible inside the agent container"; exit 1; }
echo "OK manifest_outside"
