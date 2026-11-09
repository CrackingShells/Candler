#!/bin/sh
# X35 / S3.R1-R3: tree.json inventories the workspace as path -> sha256 and lists nothing under an installed env (a dir carrying pyvenv.cfg).
# Shape (A55, F10): Candler writes R01's run-tree/1, {schema, entries[]}; file entries carry {kind:"file", path, sha256:"sha256:<hex>"}.
: "${RUN_DIR:?}"; t="$RUN_DIR/tree.json"
[ -f "$t" ] || { echo "FAIL tree_inventory: no tree.json"; exit 1; }
map='[.entries[]? | select(.kind == "file") | {key: .path, value: (.sha256 | sub("^sha256:"; ""))}] | from_entries'
jq -e '.schema == "run-tree/1" and (.entries | type == "array")' "$t" >/dev/null \
 || { echo "FAIL tree_inventory: not a run-tree/1 document"; exit 1; }
jq -e "$map"' | to_entries | length>0 and all(.[]; (.value|type=="string" and test("^[0-9a-f]{64}$")))' "$t" >/dev/null \
 || { echo "FAIL tree_inventory: file entries are not path -> sha256"; exit 1; }
keys=$(jq -r '.entries[]?.path' "$t")
printf '%s\n' "$keys" | awk -F/ '$NF=="pyvenv.cfg"{f=1} END{exit f}' || { echo "FAIL tree_inventory: pyvenv.cfg is inventoried"; exit 1; }
if [ -n "${WORKSPACE_DIR:-}" ] && [ -d "$WORKSPACE_DIR" ]; then
  for c in $(find "$WORKSPACE_DIR" -name pyvenv.cfg 2>/dev/null); do
    rel=$(dirname "$c"); rel=${rel#"$WORKSPACE_DIR"}; rel=${rel#/}
    [ -n "$rel" ] || continue
    printf '%s\n' "$keys" | awk -v p="$rel/" 'index($0,p)==1{f=1} END{exit f}' || { echo "FAIL tree_inventory: files under env root $rel are inventoried"; exit 1; }
  done
fi
echo "OK tree_inventory"
