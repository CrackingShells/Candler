#!/bin/sh
# X21: run.done exists, and was written last (nothing in the run dir changed after it).
: "${RUN_DIR:?}"
[ -f "$RUN_DIR/run.done" ] || { echo "FAIL run_sealed: no run.done"; exit 1; }
[ -f "$RUN_DIR/manifest.json" ] || { echo "FAIL run_sealed: sealed run has no manifest.json"; exit 1; }
n=$(find "$RUN_DIR" -type f ! -name run.done -newer "$RUN_DIR/run.done" | wc -l | tr -d ' ')
[ "$n" -eq 0 ] || { echo "FAIL run_sealed: $n file(s) newer than run.done"; exit 1; }
echo "OK run_sealed"
