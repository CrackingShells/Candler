# Token totals of one Claude Code stream, written as TOML.
#
#   gzip -dc agent_stream.jsonl.gz | jq -nrR -f usage_totals.jq > usage_totals.toml
#
# Lines that are not JSON are skipped. A line with no `session_id` belongs to the root session of
# the latest earlier line that has one; before any such line, to the root session of the first
# line in the stream that has one. An `assistant` or `user` line belongs to the root session
# `session_id`, or, when it has a `parent_tool_use_id`, to the session
# `<session of the line that holds that tool_use>/<parent_tool_use_id>`. Any other line with a
# `parent_tool_use_id` (a `tool_progress` heartbeat of a Bash call carries one) belongs to the
# session that holds that tool_use.
#
# [message."<session>"]: the sum over the first `assistant` line of each `message.id` in the
# stream of that line's `message.usage`; later lines of the same message count nothing.
# [session_total."<root>"]: the sum over the `result` lines of the root of their `usage`
# tokens, which Claude Code reports per turn, and the `total_cost_usd` of the last `result`
# line, which Claude Code reports as a running total.

def zero: {input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, cache_creation_tokens: 0};
def add($u): {
  input_tokens: (.input_tokens + ($u.input_tokens // 0)),
  output_tokens: (.output_tokens + ($u.output_tokens // 0)),
  cache_read_tokens: (.cache_read_tokens + ($u.cache_read_input_tokens // 0)),
  cache_creation_tokens: (.cache_creation_tokens + ($u.cache_creation_input_tokens // 0))
};
def blocks: (.message.content // []) | if type == "array" then . else [] end;

[inputs | fromjson?] as $all
| ([$all[] | select(.session_id != null) | .session_id] | first) as $first
| (reduce $all[] as $l ({cur: null, owner: {}, seen: {}, order: [], message: {}, total: {}};
  .cur = ($l.session_id // .cur)
  | ($l.session_id // .cur // $first) as $root
  | if $root == null then . else
    ($l.parent_tool_use_id // null) as $p
  | (if $p == null then $root
     elif ($l.type == "assistant" or $l.type == "user") then ((.owner[$p] // $root) + "/" + $p)
     else (.owner[$p] // $root) end) as $sid
  | (if (.order | index($sid)) == null then .order += [$sid] else . end)
  | .message[$sid] = (.message[$sid] // zero)
  | reduce ($l | blocks[] | select(.type == "tool_use") | .id) as $id (.; .owner[$id] = $sid)
  | if $l.type == "assistant" and ($l.message.id != null) and (.seen[$l.message.id] | not)
    then .seen[$l.message.id] = true | .message[$sid] |= add($l.message.usage // {})
    else . end
  | if $l.type == "result"
    then .total[$root] = ((.total[$root] // (zero + {cost_usd: null})) | add($l.usage // {}) | .cost_usd = $l.total_cost_usd)
    else . end
  end
)) as $s
| ($s.order[] | "[message.\(. | @json)]",
    ($s.message[.] | "input_tokens = \(.input_tokens)", "output_tokens = \(.output_tokens)",
      "cache_read_tokens = \(.cache_read_tokens)", "cache_creation_tokens = \(.cache_creation_tokens)"), ""),
  ($s.total | to_entries[] | "[session_total.\(.key | @json)]",
    (.value | "input_tokens = \(.input_tokens)", "output_tokens = \(.output_tokens)",
      "cache_read_tokens = \(.cache_read_tokens)", "cache_creation_tokens = \(.cache_creation_tokens)",
      (if .cost_usd == null then empty else "cost_usd = \(.cost_usd)" end)), "")
