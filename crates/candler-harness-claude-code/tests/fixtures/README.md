# Claude Code reader fixtures

Nine streams feed `guard_reader`. Each lives in `streams/<name>/` with `expected.toml`
(hand-written from `jq` over the raw lines, never from Candler code) and `usage_totals.toml`
(from `usage_totals.jq`).

| stream | source |
|---|---|
| `multi_init`, `subagent_denials`, `api_error` | golden suite `htn-creator-eval` 0.4.2, copied byte for byte |
| `mini_clean`, `mini_incomplete`, `mini_zero_mappings` | the `candler-core` mini-corpus, named by relative path in `expected.toml`; no copy here |
| `synthetic_hooks`, `synthetic_mcp`, `synthetic_orphans` | `make_synthetic.py`, from Claude Code's documented stream-json shapes |

`PROVENANCE.toml` lists the sha256 of every file here except itself and `digests.toml`.
`guard_fixture_provenance` recomputes each hash and refuses an unlisted file.

## Regenerate

```sh
# the three golden streams (read only golden directory)
uv run select_streams.py "$GOLDEN" --write
# the three synthetic streams
python3 make_synthetic.py
# usage totals of one stream
gzip -dc streams/<name>/agent_stream.jsonl.gz | jq -nrR -f usage_totals.jq > streams/<name>/usage_totals.toml
```

`digests.toml` is a ratchet over the trace file of each stream, blessed with
`CANDLER_BLESS_DIGESTS=1 cargo nextest run -p candler-harness-claude-code -E 'test(/^guard_reader::/)'`.
It refuses to rewrite an existing line unless the reader version changed.
