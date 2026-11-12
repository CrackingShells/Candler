#!/bin/sh
# The command lines of the M4 live confirmation. Every value is in session.toml and
# opencode.toml; the command line carries none. Usage: sh commands.sh <resolve|run> <session|opencode>
#   CANDLER   the built binary (required)
#   LIVE      the scratch root (default /private/tmp/claude-501/m4-live)
#   RIVAULT_LLM_INFERENCE  the key, present in the environment for run; this script never prints it
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
S=${LIVE:-/private/tmp/claude-501/m4-live}

launcher() {  # candler <verb> <file>, from an unrelated directory, in a clean environment
  mkdir -p "$S/state" "$S/store" "$S/elsewhere" && cd "$S/elsewhere"
  env -i PATH="$PATH" HOME="$HOME" XDG_STATE_HOME="$S/state" CANDLER_STORE="$S/store" \
    RIVAULT_LLM_INFERENCE="${RIVAULT_LLM_INFERENCE:-}" "${CANDLER:?}" "$1" "$HERE/$2.toml"
}

resolve() { launcher resolve "${1:?session|opencode}"; }   # prints what the file comes to; no model call
run() {  # prints the job id
  [ -n "${RIVAULT_LLM_INFERENCE:-}" ] || { echo "RIVAULT_LLM_INFERENCE is not set" >&2; exit 2; }
  launcher run "${1:?session|opencode}"
}

"$@"
