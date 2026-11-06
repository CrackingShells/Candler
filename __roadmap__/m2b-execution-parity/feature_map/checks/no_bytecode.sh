#!/bin/sh
# X (PYTHONDONTWRITEBYTECODE=1): no __pycache__ or .pyc outside an installed env in the work dir or HOME.
: "${WORKSPACE_DIR:?}"; : "${HOME_DIR:?}"
for d in "$WORKSPACE_DIR" "$HOME_DIR"; do
  roots=$(find "$d" -name pyvenv.cfg 2>/dev/null | sed 's#/pyvenv.cfg$##')
  find "$d" \( -name __pycache__ -o -name '*.pyc' \) 2>/dev/null | while read -r f; do
    skip=0; for r in $roots; do case "$f" in "$r"/*) skip=1;; esac; done
    [ $skip = 1 ] || { echo "FAIL no_bytecode: $f"; exit 1; }
  done || exit 1
done
echo "OK no_bytecode"
