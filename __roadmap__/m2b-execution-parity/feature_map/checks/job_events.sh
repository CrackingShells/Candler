#!/bin/sh
# X5 (supervision record): job dir holds job.json, children/, an append-only events.jsonl with envelope fields and increasing seq,
# and in an ended job every run.started has a run.ended or run.failed (no run left unaccounted).
: "${JOB_DIR:?}"; e="$JOB_DIR/events.jsonl"
[ -f "$JOB_DIR/job.json" ] && [ -d "$JOB_DIR/children" ] && [ -f "$e" ] || { echo "FAIL job_events: job.json, children/ or events.jsonl missing"; exit 1; }
jq -e -s '
 all(.[]; has("v") and has("seq") and has("at") and has("job_id") and has("kind") and has("payload"))
 and ([.[].seq] as $s | $s == ($s|sort) and $s == ($s|unique))
 and (.[0].kind=="job.started")
 and ( (map(.kind)|index("job.ended")) == null or
       ( [.[]|select(.kind=="run.started")|.run] as $st | [.[]|select(.kind=="run.ended" or .kind=="run.failed")|.run] as $en | ($st - $en | length)==0 ) )
' "$e" >/dev/null || { echo "FAIL job_events: envelope, seq order or run accounting broken"; exit 1; }
echo "OK job_events"
