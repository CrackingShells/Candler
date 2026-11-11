# A run's manifest.json to PROVENANCE_JSON: the names of provenance_fields.txt, nested.
#   skill_version        comparable.treatment.tag by the reference's single-v rule
#                        (htn-creator@0.2.1 and htn-creator@v0.2.3 give v0.2.1 and v0.2.3);
#                        a tag that is not a skill tag leaves it out, as the runner does
#   eval_infra.*         measured[] provenance.eval_infra.*
#   sandbox_contract     measured[] provenance.sandbox_contract (candler-container@1: not the
#                        reference's v<n> shape; a recorded deviation of provenance_fields.sh)
#   planner.sandbox.*    measured[] provenance.packages.gtpyhop.{distribution,version}
#   planner.runtime.*    not measured (recorded deviation, A59); never invented
# A value the manifest does not hold is left out, never written as null.
def m($p): [(.measured // [])[] | select(.path == $p) | .value][0];
def put($path; $v): if $v == null then . else setpath($path; $v) end;
(.comparable.treatment.tag) as $tag
| (if ($tag | type) == "string" and ($tag | test("^htn-creator@v?[0-9]+\\.[0-9]+\\.[0-9]+"))
   then "v" + ($tag | sub("^htn-creator@v?"; "")) else null end) as $skill
| m("provenance.eval_infra.tag") as $it
| m("provenance.eval_infra.commits_ahead") as $ia
| m("provenance.eval_infra.commit") as $ic
| m("provenance.eval_infra.dirty") as $id
| m("provenance.sandbox_contract") as $sc
| m("provenance.packages.gtpyhop.distribution") as $pd
| m("provenance.packages.gtpyhop.version") as $pv
| {}
| put(["skill_version"]; $skill)
| put(["eval_infra", "tag"]; $it)
| put(["eval_infra", "commits_ahead"]; $ia)
| put(["eval_infra", "commit"]; $ic)
| put(["eval_infra", "dirty"]; $id)
| put(["sandbox_contract"]; $sc)
| put(["planner", "sandbox", "distribution"]; $pd)
| put(["planner", "sandbox", "version"]; $pv)
