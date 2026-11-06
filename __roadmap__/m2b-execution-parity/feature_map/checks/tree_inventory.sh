#!/bin/sh
# X35 / S3.R1-R3: tree.json is a path -> sha256 map of the workspace, and lists nothing under an installed env (a dir carrying pyvenv.cfg).
: "${RUN_DIR:?}"; t="$RUN_DIR/tree.json"
[ -f "$t" ] || { echo "FAIL tree_inventory: no tree.json"; exit 1; }
jq -e '(if (.files? | type)=="object" then .files else (del(.schema)) end) | to_entries | length>0 and all(.[]; (.value|type=="string" and test("^[0-9a-f]{64}$")))' "$t" >/dev/null \
 || { echo "FAIL tree_inventory: not a path -> sha256 map"; exit 1; }
keys=$(jq -r '(if (.files? | type)=="object" then .files else (del(.schema)) end) | keys[]' "$t")
printf '%s\n' "$keys" | awk -F/ '$NF=="pyvenv.cfg"{f=1} END{exit f}' || { echo "FAIL tree_inventory: pyvenv.cfg is inventoried"; exit 1; }
if [ -n "${WORKSPACE_DIR:-}" ] && [ -d "$WORKSPACE_DIR" ]; then
  for c in $(find "$WORKSPACE_DIR" -name pyvenv.cfg 2>/dev/null); do
    rel=$(dirname "$c"); rel=${rel#"$WORKSPACE_DIR"}; rel=${rel#/}
    [ -n "$rel" ] || continue
    printf '%s\n' "$keys" | awk -v p="$rel/" 'index($0,p)==1{f=1} END{exit f}' || { echo "FAIL tree_inventory: files under env root $rel are inventoried"; exit 1; }
  done
fi
echo "OK tree_inventory"
