#!/bin/sh
# Prints sorted ids derived mechanically from SANDBOX_REFERENCE.md at the tag.
# S<n> per "## " header; S<invariants>.R<m> per "| I<m> |" row (invariant table = the numbered rules).
# Override REPO / TAG to point elsewhere.
REPO=${REPO:-/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion}
TAG=${TAG:-htn-creator-eval@0.4.2}
F=.claude/skills/htn-creator-workspace/infrastructure/SANDBOX_REFERENCE.md
git -C "$REPO" show "$TAG:$F" | awk '
/^## /{n++; if ($0 ~ /^## Invariants/) inv=n; print "S" n; next}
/^\| I[0-9]+ \|/{m=$2; sub(/^I/,"",m); print "S" inv ".R" m}
' | LC_ALL=C sort
