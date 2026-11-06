#!/bin/sh
# Records three real `docker inspect` outputs (clean, exit124, oom) for the B29 ends table.
# Usage: record_inspect.sh [out-dir]   (default: ./inspect next to this script)
# Needs: docker, jq, the image candler-testagent:test (assets/docker/testagent, built with
# --label candler.test=1), and the debian-slim base below (pulled by digest on first use).
# Optional: RECORD_EXTRA_LABEL=key=value adds one more label to every container.
#
# Safety: every container carries candler.job=record-<random>. The script records each cid
# and removes only recorded ids, after `docker inspect` confirms the label. No process signals,
# no name patterns, no prune.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
out="${1:-$here/inspect}"
# python:3.12-slim-bookworm (index digest), from 03-image_probe_v0.md. Coreutils `timeout` exits 124.
slim='python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df'
testagent='candler-testagent:test'
job="record-$(od -An -N6 -tx1 /dev/urandom | tr -d ' \n')"
scratch=$(mktemp -d)
cids="$scratch/cids"
: > "$cids"
mkdir -p "$out"

cleanup() {
  while read -r cid; do
    [ -n "$cid" ] || continue
    lab=$(docker inspect --format '{{index .Config.Labels "candler.job"}}' "$cid" 2>/dev/null || true)
    if [ "$lab" = "$job" ]; then docker rm -f "$cid" >/dev/null; fi
  done < "$cids"
  rm -rf "$scratch"
}
trap cleanup EXIT

# Removes anything host-specific: host paths, hostname, user name, the log path and the job label.
scrub() {
  host=$(hostname); user=$(id -un)
  jq --arg host "$host" --arg user "$user" --arg job "$job" '
    def s: if type == "string"
      then (gsub($host; "host") | gsub($user; "user") | gsub($job; "record-job")
            | if test("^/(Users|home|private|var/folders)/") then "/scrubbed" else . end)
      else . end;
    walk(s)
    | .[0].Config.Hostname = "container"
    | .[0].LogPath = "/scrubbed"
    | .[0].ResolvConfPath = "/scrubbed"
    | .[0].HostnamePath = "/scrubbed"
    | .[0].HostsPath = "/scrubbed"'
}

# record <name> <docker run args...>: run detached, wait, inspect, scrub, write.
record() {
  name="$1"; shift
  cidfile="$scratch/cid-$name"
  if [ -n "${RECORD_EXTRA_LABEL:-}" ]; then
    docker run -d --label "candler.job=$job" --label "$RECORD_EXTRA_LABEL" --cidfile "$cidfile" "$@" >/dev/null
  else
    docker run -d --label "candler.job=$job" --cidfile "$cidfile" "$@" >/dev/null
  fi
  cid=$(cat "$cidfile")
  echo "$cid" >> "$cids"
  docker wait "$cid" >/dev/null
  docker inspect "$cid" | scrub > "$out/$name.json"
}

record clean "$testagent" stream
record exit124 "$slim" timeout 1 sleep 5
record oom --memory 8m --memory-swap 8m "$slim" python3 -c 'b = b"x" * (64 << 20); print(len(b))'
echo "recorded: $(ls "$out")"
