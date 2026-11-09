#!/bin/sh
# X (token never logged or stored): CANARY appears in no file of the run's artifacts, container metadata, stream or provenance.
: "${CANARY:?}"; : "${WORKSPACE_DIR:?}"; : "${HOME_DIR:?}"
for d in "$WORKSPACE_DIR" "$HOME_DIR"; do grep -rqF -- "$CANARY" "$d" 2>/dev/null && { echo "FAIL canary_absent: found under $d"; exit 1; }; done
for f in "${STREAM_JSONL:-}" "${PROVENANCE_JSON:-}" "${INSPECT_JSON:-}"; do
  [ -n "$f" ] && [ -f "$f" ] && grep -qF -- "$CANARY" "$f" && { echo "FAIL canary_absent: found in $f (docker inspect shows container Env; pass the token per-exec, not per-container)"; exit 1; }
done
# Store side (I19): RUN_DIR / JOB_DIR / STORE_DIR, including gzip members.
for d in "${RUN_DIR:-}" "${JOB_DIR:-}" "${STORE_DIR:-}"; do
  [ -n "$d" ] && [ -d "$d" ] || continue
  grep -rqF -- "$CANARY" "$d" 2>/dev/null && { echo "FAIL canary_absent: found under $d"; exit 1; }
  for z in $(find "$d" -type f -name '*.gz' 2>/dev/null); do
    gzip -dc "$z" 2>/dev/null | grep -qF -- "$CANARY" && { echo "FAIL canary_absent: found inside $z"; exit 1; }
  done
done
echo "OK canary_absent"
