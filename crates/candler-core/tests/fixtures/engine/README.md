# Fixtures of the engine guards

Read by `guard_engine`. The guard fills the `@...@` tokens at run time and runs the plug-in for real.

| Path | Holds |
|---|---|
| `plugin.toml` | the manifest of the `engine` plug-in; it runs the helper from `python/candler-evaluator` under `uv run --frozen`, with `PYTHONPATH` set to the helper's example plug-ins and `kinds/`. `@EXAMPLES@` and `@KINDS@` are absolute paths |
| `kinds/engine_fixture.py` | the plug-in module: the helper's `ok` (synthetic) and `echo` kinds, the `fail` kind and the `counter` kind; `counter` keeps its count in the file its `file` param names. `echo` gets the needs of the parity suite so that it echoes the live context |
| `suite_dependencies.toml` | five entries: one with an `applies_to` miss, a `fail` with a dependent, a `pass` with a dependent |
| `plugin_bare.toml` | the same manifest with a bare `python -m candler_evaluator ...` command: it runs only inside the activated environment, with no `uv run` |
| `suite_dependencies.json` | `suite_dependencies.toml` written as JSON; the engine reads `.json` files as JSON |
| `suite_echo.toml` | one `echo` entry |
| `suite_counter.toml` | one `counter` entry; `@COUNT@` is the count file |
| `kinds/engine_second.py` | the plug-in module of the second evaluator, `second`, with the `ok` kind only |
| `plugin_second.toml` | the manifest of `second`; the same command as `plugin.toml`, with the module `engine_second` |
| `suite_batching.toml` | three independent entries: B1 and B2 of `engine`, C1 of `second` |
| `suite_deadlines.toml` | two `ok` entries in one request, with item deadlines of 40 s and 5 s |
| `suite_order.toml` | N1 with an `applies_to` miss and its dependent N2; D1, an `echo`, listed before its dependency P3 |
