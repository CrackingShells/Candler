#!/bin/sh
# S1, X: PROVENANCE_JSON carries every reference provenance field (dotted names), shapes sane.
: "${PROVENANCE_JSON:?PROVENANCE_JSON unset}"
jq -e '
  ["skill_version","eval_infra.tag","eval_infra.commits_ahead","eval_infra.commit","eval_infra.dirty",
   "sandbox_contract","planner.runtime.distribution","planner.runtime.version",
   "planner.sandbox.distribution","planner.sandbox.version"] as $want
  | [paths | map(tostring) | join(".")] as $have
  | ($want | map(select(. as $w | $have | any(. == $w) | not))) as $missing
  | ($missing | length == 0)
    and (.skill_version | type == "string" and test("^v[0-9]+\\.[0-9]+\\.[0-9]+"))
    and (.sandbox_contract | type == "string" and test("^v[0-9]+$"))
    and (.eval_infra.dirty | type == "boolean")' "$PROVENANCE_JSON" >/dev/null \
  || { echo "FAIL provenance_fields: missing/ill-shaped field(s) in $PROVENANCE_JSON"; exit 1; }
jq -e '.eval_infra.dirty == false' "$PROVENANCE_JSON" >/dev/null \
  || echo "WARN eval_infra.dirty=true: reference treats such a run as non-comparable"
echo "OK provenance_fields"
