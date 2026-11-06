#!/bin/sh
# S4 (uid residual; contract names a container or separate uid the honest end state): agent does not run as root.
: "${EXEC:?}"
u=$($EXEC id -u) && [ "$u" != 0 ] || { echo "FAIL non_root: agent uid is ${u:-unknown}"; exit 1; }
echo "OK non_root (uid $u)"
