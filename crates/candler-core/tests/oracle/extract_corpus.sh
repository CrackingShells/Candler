#!/usr/bin/env bash
# Usage: extract_corpus.sh <artifact repo> <dest>
# Materialises campaign/20260806-cont-train of tag candler-m2a-golden under <dest>
# from a `git archive`, then recreates the empty `outputs/` directories git drops.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <artifact repo> <dest>" >&2
  exit 2
fi
repo=$1
dest=$2
tag=candler-m2a-golden
root=campaign/20260806-cont-train

if [ ! -d "$dest" ]; then
  echo "refusing: $dest is not a directory" >&2
  exit 1
fi
if [ -n "$(ls -A "$dest")" ]; then
  echo "refusing: $dest is not empty" >&2
  exit 1
fi

git -C "$repo" archive "$tag" "$root" | tar -x -C "$dest"

for run in "$dest/$root"/*/_run_cache/with_skill/run-*; do
  if [ -d "$run" ] && [ ! -d "$run/outputs" ]; then
    mkdir "$run/outputs"
    echo "recreated empty directory: $run/outputs"
  fi
done
