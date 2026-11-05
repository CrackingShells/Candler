#!/bin/sh
# A process tree that ignores TERM: this shell, one grandchild in its group and one grandchild
# that leaves the group for a session of its own. Each process id is appended to the file named
# by the first argument as `<label> <pid>`, and the line `ready` follows once all three exist.
# Usage: tree.sh PIDFILE
trap '' TERM
sleep 60 &
echo "group $!" >> "$1"
perl -MPOSIX -e 'POSIX::setsid(); open(my $f, ">>", $ARGV[0]) or die; print $f "detached $$\n"; close $f; exec q(sleep), q(60)' "$1" &
echo "leader $$" >> "$1"
until grep -q '^detached ' "$1"; do sleep 0.05; done
echo ready >> "$1"
wait
