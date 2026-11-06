#!/bin/sh
# X7: the agent's stderr is archived with the run (raw/stderr.jsonl.gz, a raw-frames file with a header line).
: "${RUN_DIR:?}"
f="$RUN_DIR/raw/stderr.jsonl.gz"
[ -f "$f" ] || { echo "FAIL stderr_kept: $f missing"; exit 1; }
gzip -t "$f" 2>/dev/null || { echo "FAIL stderr_kept: not valid gzip"; exit 1; }
gzip -dc "$f" | head -n 1 | jq -e '.kind=="header" and .stream=="stderr"' >/dev/null 2>&1 || { echo "FAIL stderr_kept: first line is not a stderr header"; exit 1; }
echo "OK stderr_kept"
