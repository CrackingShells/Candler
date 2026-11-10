# agent_env leaf: findings

## P3

**Question.** Do the six HTN requirements, compiled from an experiment's requirements file instead of from `stable.in`, give the same lock bytes? That decides whether live HTN cell digests move.

**Method.** The `COMPILE_SCRIPT` invocation of `isolation/image.rs` (`uv pip compile --python-version 3.12 --python-platform "$(uname -m)-unknown-linux-gnu" --no-header`), run in a control container of the pinned base (`python:3.12-slim-bookworm@sha256:935c3c5b...` plus `uv:0.9@sha256:538e0b39...`, the same layers as `assets/docker/base`), network `bridge`, `UV_NO_CACHE=1`. Two source files with the same six lines (`stable.in` as it is at b017d31, and a copy named `requirements.txt`), compiled in the same hour. The scripts live outside the repository and are not committed.

**Result.**

| | `stable.in` | `requirements.txt` |
|:--|:--|:--|
| lines | 121 | 121 |
| resolved packages | 40 | 40 |
| sha256 of the bytes | 23968d10... | ffcbefc7... |
| sha256 with `#` lines removed | f8445e0a... | f8445e0a... |

The pins are identical. The six differing lines are all `# via -r lock-src/<file name>` annotations, which name the source file. Nothing else differs.

**Decision.** The lock digest is the sha256 of the bytes, and it feeds `isolation.image`, hence the cell digest. So an HTN experiment that declares the six lines in `isolation.build.requirements` gets a different lock digest from the b017d31 one, whatever name it gives the file, and every live HTN cell digest moves. Two causes add up:
- the annotations name the new source file;
- the compiler resolves the newest versions at compile time (`gtpyhop-core==2.0.2`, `mcp==2.3.0` today), so a lock compiled later differs from a recorded one whatever its file names are.

Golden-import and parity-fixture digests do not depend on a compile and are unaffected (G5c). The smoke report lists every live HTN cell digest that moved, and attributes the move to the lock bytes, not to the key's definition.
