#!/bin/sh
# S3.R12: the post-run work dir contains no pyvenv.cfg (the sandbox venv was reclaimed before the keep decision).
: "${WORKSPACE_DIR:?}"
f=$(find "$WORKSPACE_DIR" -name pyvenv.cfg -print -quit)
[ -z "$f" ] || { echo "FAIL workspace_no_pyvenv: $f"; exit 1; }
echo "OK workspace_no_pyvenv"
