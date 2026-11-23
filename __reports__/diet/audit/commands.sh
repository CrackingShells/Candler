#!/bin/sh
# The commands of the e2e audit, one function each, run in order by the leaf (see the findings
# report). Usage: sh commands.sh <step>. Every record goes to $A/records/<step>.json.
#   CANDLER  the built binary   RIVAULT_LLM_INFERENCE  the key (never printed)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
A=${AUDIT_STORE:-$HOME/candler-diet-audit-store}
export CANDLER_STORE="$A/store" XDG_STATE_HOME="$A/state"
C=${CANDLER:-$REPO/target/release/candler}
mkdir -p "$A/store" "$A/state" "$HERE/records"
rec() { name=$1; shift; "$@" > "$HERE/records/$name.json" 2> "$HERE/records/$name.err" || echo "exit $? ($name)"; }

stage() {  # the treatment repository, and the plug-in directory with its paths filled in
  [ -d "$A/treatment/.git" ] || { cp -R "$HERE/treatment" "$A/treatment" && git -C "$A/treatment" init -q \
    && git -C "$A/treatment" add -A && git -C "$A/treatment" -c user.name=audit -c user.email=a@b commit -qm audit \
    && git -C "$A/treatment" tag v1; }
  rm -rf "$A/plugins"; mkdir -p "$A/plugins" "$A/project"
  for p in audit_file audit_second; do mkdir -p "$A/plugins/$p"; cp "$HERE/plugins/$p/$p.py" "$A/plugins/$p/";
    sed "s#@REPO@#$REPO#g;s#@PLUGINS@#$A/plugins#g" "$HERE/plugins/$p/plugin.toml" > "$A/plugins/$p/plugin.toml"; done
  printf '[store]\nroot = "%s/store"\n\n[plugins]\ndirs = ["%s/plugins/audit_file", "%s/plugins/audit_second"]\n' "$A" "$A" "$A" > "$A/project/candler.toml"
}
inproj() { cd "$A/project"; "$@"; }

resolve() { rec resolve "$C" resolve "$HERE/experiment.toml"; }
"$@"
