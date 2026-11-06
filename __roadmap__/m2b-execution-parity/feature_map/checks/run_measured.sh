#!/bin/sh
# X29: the run's end is measured and recorded (run.ended_by, run.exit); with JOB_DIR, a run end event carries ended_by.
: "${RUN_DIR:?}"
jq -e '(.measured|map(.path)) as $p | ($p|index("run.ended_by")) != null and ($p|index("run.exit")) != null' "$RUN_DIR/manifest.json" >/dev/null 2>&1 \
 || { echo "FAIL run_measured: measured[] lacks run.ended_by or run.exit"; exit 1; }
if [ -n "${JOB_DIR:-}" ] && [ -f "$JOB_DIR/events.jsonl" ]; then
  jq -e 'select(.kind=="run.ended") | .payload | has("ended_by")' "$JOB_DIR/events.jsonl" 2>/dev/null | grep -q true \
   || { echo "FAIL run_measured: no run.ended event with ended_by"; exit 1; }
fi
echo "OK run_measured"
