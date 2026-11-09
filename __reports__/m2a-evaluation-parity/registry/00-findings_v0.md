# Registry: first dispatch stopped on two spec defects (v0)

The implementer wrote no code and reported the defects below; the coordinator ruled on them, as the **Rulings** section at the end records.

1. Commits: none. Branch m2a/registry still at f5c2533, worktree clean.
2. Gates: none run (nothing to gate).
3. Guards: none (REG1, REG2 not written).
4. Records: none.
5. Findings:
 D1 (blocks Step 1 and REG1): check_suite cannot get KindSpecs without spawning. PluginManifest has only schema,name,version,protocol,command,environment. KindSpec {kind, params_schema, needs, default_deadline_ms} exists only in DescribeResult.kinds (the `describe` RPC, i.e. a spawn). Kind existence, params schema and needs all depend on it. R03 sequence diagram has check_suite before EnvironmentStep and before describe, and I14 says validation spawns nothing.
 D2 (blocks Step 2 and REG2): register_evaluation_steps(resolver, &Registry, env_root) has no Supervisor, but PreflightStep::run is BoxFuture<'static> over Arc<ResolveContext> only, so the step must own an Arc<Supervisor>. Building a private Supervisor inside the registry would bypass the binary's shared supervisor (ctrl-c cannot stop uv).
 Notes: the manifest's environment is Uv{project, lock} RelPaths, so the digest should hash those files, not hardcoded names. Supervisor spawn does env_clear, so uv needs PATH/HOME forwarded explicitly (and PATH is how the REG2 stub is found).
 Suggested rulings: D2 add `supervisor: Arc<Supervisor>` to register_evaluation_steps. D1 pick one: (a) Registry::with_kinds(&Name, Vec<KindSpec>) fed from describe, so check_suite runs after describe (spawns happen only in client); (b) add `kinds` to the manifest (schemas change); (c) a kinds sidecar file.
 Re-dispatch after rulings.

## Rulings (coordinator, 2026-11-06)
- **D1, route (a).** R01 §Operation API makes `describe` part of the preflight, so the `KindSpec`s come from `DescribeResult`. `check_suite(&self, suite, described: &BTreeMap<Name, DescribeResult>)` is pure and also refuses a `describe` that disagrees with its manifest. The engine calls `Client::describe` and then `check_suite`. Route (b) would change a schema for data `describe` already carries; route (c) would duplicate it.
- **D2.** `register_evaluation_steps` takes `supervisor: Arc<Supervisor>`, the binary's shared one.
- **Notes adopted.** The lock digest hashes the files `PluginEnvironment::Uv { project, lock }` names. The step forwards `PATH` and `HOME` and sets `UV_PROJECT_ENVIRONMENT`. Child records go under `<env root>/<digest>/`.
