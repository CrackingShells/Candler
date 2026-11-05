#!/bin/sh
# A fake harness for the seams guard: it plays back a recorded stream, so that one run crosses
# the supervisor, the frame writer and the store without a real harness. It writes the stream
# named by its first argument to standard output, one line to standard error, and exits 0, or 1
# if the stream cannot be played. It never prints its environment: a secret that reaches the
# store has to come from the supervisor.
# Usage: fake_harness.sh STREAM.jsonl.gz
gunzip -c "$1" || exit 1
echo "fake harness: stream played back" >&2
exit 0
