# MCP specification fixtures

The `Tool` definition of the latest published MCP specification, and tool objects the specification
gives as examples. They are inputs to the catalog snapshot tests: every tool here is a valid MCP tool
object, so a snapshot schema that rejects one is too strict.

## Provenance

| | |
|---|---|
| Repository | `modelcontextprotocol/modelcontextprotocol` (GitHub) |
| Commit | `ccaf9102ec5eb656eab50ca8333ef49dcd259c26` (`main`, 2026-10-27T17:10:04Z) |
| Version | `2026-07-28`, the latest dated version under `schema/` that is not `draft` |
| Fetched with | `gh api -H 'Accept: application/vnd.github.raw+json' 'repos/modelcontextprotocol/modelcontextprotocol/contents/<path>?ref=ccaf9102ec5eb656eab50ca8333ef49dcd259c26'` |
| Licence | `LICENSE` of the repository, vendored here. The project is moving from MIT to Apache-2.0: new code and specification contributions are Apache-2.0, documentation outside the specification is CC-BY-4.0, and contributions without relicensing consent stay MIT. |

## Files

| File | Source path at the commit | Blob | Size |
|---|---|---|---:|
| `mcp-2026-07-28.schema.json` | `schema/2026-07-28/schema.json` | `213c58f6d9a1c2ce6ad055afe90bbdb095a29ee8` | 181,474 B |
| `LICENSE` | `LICENSE` | `4a93985763241755401a10678395303de4e720ba` | 12,227 B |

`mcp-2026-07-28.schema.json` is JSON Schema 2020-12. Its `$defs.Tool` has the properties `_meta`,
`annotations`, `description`, `icons`, `inputSchema`, `name`, `outputSchema` and `title`; `inputSchema`
and `name` are required. `inputSchema` and `outputSchema` are open objects: `outputSchema` may
be any JSON Schema, an array schema included.

## `tools/`

One file per distinct tool example in the specification. Files taken from the schema examples are
byte for byte; files taken from the documentation are the JSON block of the page, re-indented with
two spaces.

| File | Origin | Blob of the source file |
|---|---|---|
| `tool-with-array-output-schema.json` | `schema/2026-07-28/examples/Tool/tool-with-array-output-schema.json` | `8c7edec6237a12b1c76fb3af13c9430212840306` |
| `tool-with-composition-input-schema.json` | `schema/2026-07-28/examples/Tool/tool-with-composition-input-schema.json` | `7c7253af9f356759cff39c8f9746dfa16c212a8b` |
| `with-default-2020-12-input-schema.json` | `schema/2026-07-28/examples/Tool/with-default-2020-12-input-schema.json` | `d79a00eeaa7cca70a61a9606148ec5ee433d2512` |
| `with-explicit-draft-07-input-schema.json` | `schema/2026-07-28/examples/Tool/with-explicit-draft-07-input-schema.json` | `698d95b86549291e0acde11955f64e60b6e7eb71` |
| `with-no-parameters.json` | `schema/2026-07-28/examples/Tool/with-no-parameters.json` | `04a3a4e956385db9a7a0533f97351dc1f2a14d20` |
| `with-output-schema-for-structured-content.json` | `schema/2026-07-28/examples/Tool/with-output-schema-for-structured-content.json` | `a1469834247527890c22f3e23ec3015ca44910b8` |
| `docs-get-weather-with-icons.json` | `docs/specification/2026-07-28/server/tools.mdx`, the `tools/list` response (also in `examples/ListToolsResult/`) | `449020f54a6582122607b4869129bec5f1035f37` |
| `docs-execute-sql-x-mcp-header.json` | `tools.mdx`, "Example tool definition with `x-mcp-header`" | same |
| `docs-list-users-array-output.json` | `tools.mdx`, "Example tool with array output schema" (the schema example of the same name carries property descriptions; this one does not) | same |
| `full.json` | Written from `$defs.Tool`: every property of `Tool`, every property of `ToolAnnotations` and of `Icon`, and a `_meta` with two valid keys | none |

The other tool blocks of `tools.mdx` (`get_weather_data`, `calculate_sum` twice, `get_current_time`)
equal the schema examples above and are not repeated.

Check: `for f in tools/*.json; do jq -e .name "$f" >/dev/null || exit 1; done`.
