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

# The audit, in the order it ran (each record is saved beside this file in records/).
# $S is the job id a step prints; ps/watch/kill read $A/store through the exported variables.
resolve() { rec resolve "$C" resolve "$HERE/experiment.toml"; }
run_all()   { rec run_1 "$C" run "$HERE/experiment.toml"; }                          # 8 runs, finished in 33 s
run_kill()  { rec run_2 "$C" run --set replicates=3 "$HERE/experiment.toml"; }        # then: candler kill -y <job>
run_resume(){ rec run_3 "$C" run --set replicates=3 "$HERE/experiment.toml"; }        # resumes: only the missing runs
run_variant(){ rec run_4_variant "$C" run --set replicates=1 --set budget.per_run.wall_s=601 "$HERE/experiment.toml"; }
evaluate()  { ( cd "$A/project" && for s in suite_file suite_second; do rec evaluate_$s "$C" evaluate "$HERE/$s.toml"; done ); }
rates()     { ( cd "$A/project" && for e in audit-file@0.1.0 audit-second@0.1.0; do rec show_rates_$e "$C" show --rates --evaluator $e; done ); }
ingest()    { rec ingest "$C" ingest --rules "$REPO/crates/candler/tests/fixtures/cli/mini-corpus-rules.toml" "$REPO/crates/candler-core/tests/fixtures/mini-corpus"; }
analyze()   { ( cd "$A/project" && rec analyze_second "$C" analyze --evaluator audit-second@0.1.0 ); }
e2e()       { ( cd "$REPO" && cargo nextest run --workspace --run-ignored all --no-fail-fast -E 'test(/e2e/) | test(/i8/)' ) > "$HERE/records/e2e_guards.txt" 2>&1; }
# ps, kill and the job event copies: candler ps; candler kill -y <job>; cp $A/store/jobs/<job>/events.jsonl records/
"$@"
