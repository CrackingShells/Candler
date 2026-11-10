#!/bin/sh
# The command lines of the M2c re-validation smoke (Step 2). Every value is in experiment.toml;
# the command line carries none. Usage: sh commands.sh <stage|run|leak|project|checks|leftovers> [arg]
#   CANDLER   the built binary (required for run, leak)
#   SMOKE     the scratch root (default /private/tmp/claude-501/m2c-smoke)
#   RIVAULT_LLM_INFERENCE  the key, present in the environment; this script never prints it
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
S=${SMOKE:-/private/tmp/claude-501/m2c-smoke}
T=$S/treatment                    # clone of PCfVW/mcp-python-ingestion
SKILL_TAG='htn-creator@0.2.7'     # the treatment's tag (B38: a smoke treatment is a skill tag)
SERVERS_TAG='htn-creator-eval@0.4.2'   # the golden reference, which holds the rikyu_mcp servers
FM=$HERE/../../../__roadmap__/m2b-execution-parity/feature_map

stage() {  # exp/ holds the experiment and the files it names, built from the clone
  rm -rf "$S/exp" "$S/stage.tmp" && mkdir -p "$S/exp/inputs/servers" "$S/exp/inputs/scripts" "$S/stage.tmp" "$S/elsewhere"
  git -C "$T" archive "$SERVERS_TAG" .claude/skills/htn-creator-workspace/servers/rikyu_mcp | tar -x -C "$S/stage.tmp"
  for f in "$S"/stage.tmp/.claude/skills/htn-creator-workspace/servers/rikyu_mcp/*.py; do
    case ${f##*/} in __*|*_test.py) ;; *) cp "$f" "$S/exp/inputs/servers/" ;; esac
  done
  git -C "$T" archive "$SKILL_TAG" .claude/skills/htn-creator/scripts | tar -x -C "$S/stage.tmp"
  cp -R "$S/stage.tmp/.claude/skills/htn-creator/scripts/." "$S/exp/inputs/scripts/"
  # the reference's write_sandbox_pyproject, byte for byte
  printf '[project]\nname = "htn-eval-sandbox-project"\nversion = "0.0.0"\nrequires-python = ">=3.10"\n' > "$S/exp/inputs/pyproject.toml"
  # the six packages of the reference's stable set (assets/docker/env/stable.in at b017d31)
  printf '%s\n' 'libcst>=1.1.0,<2' 'jsonschema>=4.0' 'python-arango>=8.2.2,<9' 'python-dotenv>=1.0,<2' \
    'mcp>=1.18.0' 'gtpyhop-core>=2.0.0,<3' > "$S/exp/requirements.in"
  cp "$HERE/experiment.toml" "$S/exp/experiment.toml"
  rm -rf "$S/stage.tmp"
}

launcher() {  # candler run <file>, from an unrelated directory, in a clean environment
  [ -n "${RIVAULT_LLM_INFERENCE:-}" ] || { echo "RIVAULT_LLM_INFERENCE is not set" >&2; exit 2; }
  mkdir -p "$S/state" "$S/elsewhere" && cd "$S/elsewhere"
  env -i PATH="$PATH" HOME="$HOME" XDG_STATE_HOME="$S/state" \
    RIVAULT_LLM_INFERENCE="$RIVAULT_LLM_INFERENCE" "${CANDLER:?}" run "$1"
}

run() { launcher "${1:-$S/exp/experiment.toml}"; }        # prints the job id

leak() {  # X32: same cell, a prompt that copies a declared input to an output path
  awk '/^"prompt.template" = /{print "\"prompt.template\" = \"Run `cp pyproject.toml domain.py`, then stop. Non-interactive: do not ask for confirmation.\""; next} {print}' \
    "$S/exp/experiment.toml" > "$S/exp/leak.toml"
  launcher "$S/exp/leak.toml"
}

project() { jq -f "$HERE/projection.jq" "$1"; }            # a run's manifest.json to PROVENANCE_JSON

checks() {  # the 31 checks as committed; the caller sets the env vars of checks/README.md
  for c in $(jq -r '.[]|select(.check!=null)|.check' "$FM/feature_map.json" | sort -u); do
    sh "$FM/checks/$c.sh" >"$S/check-$c.out" 2>&1 && r=0 || r=$?
    printf '%s\t%s\t%s\n' "$c" "$r" "$(head -n1 "$S/check-$c.out")"
  done
}

leftovers() {  # $1 = job id: containers and networks labelled for the job; both must print nothing
  docker ps -a --filter "label=candler.job=$1" -q
  docker network ls --filter "label=candler.job=$1" -q
}

"$@"
