#!/bin/sh
# X (--setting-sources project,local): no user-global skills, plugins, MCP servers, memory or credential dirs reach the agent.
: "${STREAM_JSONL:?}"; : "${EXEC:?}"; : "${HOME_DIR:?}"
jq -nR '[inputs | fromjson? | select(type == "object" and .type == "system" and .subtype == "init")] | first
  | ((.mcp_servers // []) | length == 0) and ((.plugins // []) | length == 0)' "$STREAM_JSONL" | grep -qx true \
  || { echo "FAIL settings_sources_isolated: init lists MCP servers or plugins"; exit 1; }
for p in .claude/skills .claude/plugins .claude/CLAUDE.md .claude/settings.json .ssh .aws .gnupg .docker .netrc; do
  [ -e "$HOME_DIR/$p" ] && { echo "FAIL settings_sources_isolated: HOME has $p"; exit 1; }
  $EXEC sh -c "test -e \"\$HOME/$p\"" && { echo "FAIL settings_sources_isolated: container HOME has $p"; exit 1; }
done
echo "OK settings_sources_isolated"
