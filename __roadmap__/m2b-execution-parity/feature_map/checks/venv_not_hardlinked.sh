#!/bin/sh
# X (UV_LINK_MODE=copy): no venv file shares an inode with anything else (so an in-place edit cannot mutate another venv or a cache).
: "${EXEC:?}"
V=$($EXEC sh -c 'printf %s "$VIRTUAL_ENV"')
f=$($EXEC sh -c "find '$V' -type f -links +1 -print -quit")
[ -z "$f" ] || { echo "FAIL venv_not_hardlinked: $f has >1 hard link (package-owned links can be false positives; inspect)"; exit 1; }
echo "OK venv_not_hardlinked"
