#!/bin/sh
# X (--setting-sources project,local): no user-global skills, plugins, MCP servers, memory or credential dirs reach the agent.
# Plugins that ship inside the pinned harness (path "builtin", source "<name>@builtin") are allowed (A55, F12); any other plugin fails.
: "${STREAM_JSONL:?}"; : "${EXEC:?}"; : "${HOME_DIR:?}"
jq -nR '[inputs | fromjson? | select(type == "object" and .type == "system" and .subtype == "init")] | first
  | ((.mcp_servers // []) | length == 0) and ((.plugins // []) | all(.[]; .path == "builtin" and (.source | type == "string" and endswith("@builtin"))))' "$STREAM_JSONL" | grep -qx true \
  || { echo "FAIL settings_sources_isolated: init lists MCP servers or a non-builtin plugin"; exit 1; }
# The harness writes its builtin plugins' state under HOME/.claude/plugins/store/<name>_builtin-<hex>.json while it runs
# (seen on 2.1.311); that is not a user-global plugin. Any other entry there fails. HOME is the bind the container sees.
if [ -d "$HOME_DIR/.claude/plugins" ]; then
  other=$(cd "$HOME_DIR/.claude/plugins" && find . -type f ! -path './store/*_builtin-*.json' | head -1)
  [ -z "$other" ] || { echo "FAIL settings_sources_isolated: HOME has a non-builtin plugin file .claude/plugins/${other#./}"; exit 1; }
fi
for p in .claude/skills .claude/CLAUDE.md .claude/settings.json .ssh .aws .gnupg .docker .netrc; do
  [ -e "$HOME_DIR/$p" ] && { echo "FAIL settings_sources_isolated: HOME has $p"; exit 1; }
  $EXEC sh -c "test -e \"\$HOME/$p\"" && { echo "FAIL settings_sources_isolated: container HOME has $p"; exit 1; }
done
echo "OK settings_sources_isolated"
