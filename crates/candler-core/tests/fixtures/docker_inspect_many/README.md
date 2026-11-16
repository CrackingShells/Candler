# Inspect of several containers, one of them gone

`docker container inspect --format '{{json .}}' A MISSING B C` on Docker Engine 29.9.2 (Docker
Desktop, macOS), measured on three created, never started containers and a fourth id no
container has (sixty-four `f`).

- `three_ids_one_missing.stdout`: one JSON object per line, for A, B and C in the order asked; the
  missing id has no line.
- `three_ids_one_missing.stderr`: one line, `Error response from daemon: No such container: <id>`.
- The exit code is 1 whenever any id is missing, and 0 when all are found.
- The order asked is A, the missing id, B, C: `A`, `B`, `C` are the `Id` of the first, second and
  third line.
