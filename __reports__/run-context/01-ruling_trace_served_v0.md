# Ruling: the trace, the raw streams and the manifest are served (v0)

## What is replaced

The M2a ruling for `run_context` (`__reports__/m2a-evaluation-parity/l1/00-rulings_v0.md:75`)
accepted "`needs.trace` is ignored". That ruling is replaced. When a kind of a request declares
`needs.trace`, `needs.raw` or `needs.manifest`, the request is handed the stored file, and the
context says which one by its digest.

## Why

- **The README's evaluation half names traces.** "Grade traces and artifacts, fresh or already on
  disk" cannot hold while a declared need for the trace is accepted and dropped. AGENTS.md calls a
  second, more lenient reading of a declaration a defect even when nothing reaches it yet; ignoring
  a declared need is that reading.
- **The declarations state what a plug-in is served.** Candler has no blinding concept. A kind
  lists its needs in `describe`, and the engine builds one context per request from the union of
  the needs of the kinds of the entries the request sends, so what a plug-in is served is the
  union of what those kinds declare. Every kind in the request can read any part another one
  declared. Candler serves exactly that and refuses what it cannot serve, so what a plug-in was
  served is on record in the declarations of the kinds it was sent. Serving the trace adds no
  hidden channel.
- **A sealed run never changes.** "Sealed" means complete and protected from overwrite, so the
  digest taken when a stored file is chosen for a context holds for the whole evaluation, and the
  outcome-reuse key can carry it.

## What follows

- A part the run does not store (the trace or the raw streams of a run the importer could not
  read, or of a run sealed after an interruption) is refused with `ContextError::PartMissing`,
  code `evaluation.run_part_missing`, and the whole request is refused with it. A stored trace can
  hold its header alone, when the reader failed or the session was silent, and it is served as
  stored. The manifest is written when the run directory is made, so every run holds it.
- Every stored file reaches a workspace through one copy routine, so a served part has the
  guarantees an artifact has: a private copy, `0444`, checked against its digest.
