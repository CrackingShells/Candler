#!/bin/sh
# X (hook allow-list): the usual pseudo-devices stay usable for the agent.
: "${EXEC:?}"
$EXEC sh -c 'echo x >/dev/null && head -c1 /dev/urandom >/dev/null && head -c1 /dev/zero >/dev/null && echo x >/dev/stderr' 2>/dev/null \
  || { echo "FAIL benign_devices: /dev/null|urandom|zero|stderr unusable"; exit 1; }
echo "OK benign_devices"
