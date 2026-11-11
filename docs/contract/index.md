# Contract pages

One page per surface an author writes or a plug-in speaks. Each page holds the sections Surface, Version, Fields, Refusals and Notes, in that order. Version and Fields are generated from the committed schema files in `crates/candler-schemas/schema/`.

## Pages

| Page | Surface | Written by | Wire tag |
|---|---|---|---|
| [plugin-manifest.md](plugin-manifest.md) | Plug-in manifest | plug-in author | `plugin-manifest/1` |
| [suite-file.md](suite-file.md) | Suite file | suite author | `suite/1` |
| [evaluator-protocol.md](evaluator-protocol.md) | Evaluator protocol | Candler and the evaluator plug-in | none |
| [outcome-record.md](outcome-record.md) | Outcome record | evaluator plug-in; Candler fills the rest | none |
| [experiment-file.md](experiment-file.md) | Experiment file | experimenter | `experiment/1` |
| [evaluator-helper.md](evaluator-helper.md) | Evaluator helper | evaluator kind author | none |
| [driver-traits.md](driver-traits.md) | Driver traits | driver author, by hand | none |

`driver-traits.md` is written by hand and is not generated.

## Field classes

Every row of a Fields table has one class. A `required` followed by a variant in parentheses, such as `required (value.kind = fraction)`, is required only when the object takes that variant.

| Class | Meaning |
|---|---|
| `required` | An author sets it, and the object's `required` lists it. |
| `optional` | An author may set it, and `required` does not list it. |
| `candler` | Candler writes it; an author never sets it. |
| `free` | Stored and never read; it needs no schema of its own. |

## Compatibility

<!-- versions -->
| Line | Version | Manifest |
|---|---|---|
| core | `v0.6.1` | `Cargo.toml` |
| wire schemas | `schemas@0.6.0` | `crates/candler-schemas/Cargo.toml` |
| evaluator helper | `candler-evaluator@0.1.0` | `python/candler-evaluator/pyproject.toml` |
<!-- /versions -->

## Other refusals

Surfaces that name no page of their own: configuration, resolving, running, isolation, the store, jobs, evaluation and importing. The path is an RFC 6901 pointer into the document at fault, or a file.

<!-- generated:refusals -->
| Code | When | Path at fault | Side to fix |
|---|---|---|---|
| `analysis.ambiguous_evaluator_name` | A bare version is held by several evaluators; write `name@version`. | - | config |
| `analysis.experiment_disagrees` | The factors of the file given with `--experiment` differ from the factors the manifests of a stratum record, so the file is not the one that made these runs. | - | experiment |
| `analysis.experiment_unreadable` | The file given with `--experiment` cannot be read as an experiment file. | - | experiment |
| `analysis.factor_broader_than_variation` | A stratum's manifests record a factor that is a strict prefix of a path where its cells differ, so the record cannot be trusted to declare the variation. | - | environment |
| `analysis.mixed_evaluator_versions` | The store holds outcomes of several evaluator versions and the request names none; name one with `--evaluator`. | - | config |
| `analysis.no_outcomes` | The store holds no outcome record, so there is nothing to analyse; run `evaluate` first. | - | environment |
| `analysis.unknown_evaluator` | The evaluator, or the version, named has no outcome in the store. | - | config |
| `catalog.name_empty` | A server or tool name in the catalog an image prints is empty. | - | environment |
| `catalog.name_repeated` | A server or tool name in the catalog an image prints is repeated within its scope. | - | environment |
| `catalog.schema_tag_mismatch` | The tool catalog an image prints carries another `schema` marker than `tool-catalog-snapshot/1`. | `/schema` | environment |
| `cli.usage_invalid` | The command line is not one the parser accepts: an unknown option, a missing argument, or a value such as a `--set` without `=`. | - | config |
| `config.ambiguous_run` | More than one run of the store has a key that starts with the prefix given to `inspect`. | - | config |
| `config.cwd_not_absolute` | The working directory is not an absolute path, so no ancestor can be searched for a configuration. | - | config |
| `config.evaluator_invalid` | `--evaluator`, `--from` or `--to` holds text that is not `name@version` with a SemVer 2.0 version. | - | config |
| `config.experiment_unreadable` | The experiment file given to `run` or `resolve` does not exist or cannot be read. | - | config |
| `config.file_unreadable` | A configuration file exists and cannot be read. | - | config |
| `config.import_field_unset` | A field the import rules need is not set. | - | config |
| `config.import_rule_invalid` | A rule of the import rules is not valid. | - | config |
| `config.import_rules_invalid` | The import rules file given to `ingest --rules` is not the rules format: it is not TOML, a field is missing, or a field is not one the format has. | - | config |
| `config.inspect_target_invalid` | The target given to `inspect` is none of a run directory, a prefix of a run key of at least 6 characters, or a job id. | - | config |
| `config.kill_not_confirmed` | `kill` asked for a confirmation and was not answered `y` or `yes`; `-y` skips the question. | - | config |
| `config.machine_wide_key` | A project configuration sets a machine-wide key, which only the user configuration may set. | The key in the project configuration file named in the message. | config |
| `config.no_evaluator_slots` | `slots.evaluators` is 0 in the user configuration, so the pool that evaluators draw from has no slot. | - | config |
| `config.no_plugin_dirs` | `evaluate` is run where no `candler.toml` at or above the working directory lists a plug-in directory under `plugins.dirs`. | - | config |
| `config.no_store` | Not raised today: a verb that is given no store root takes the default store, and a store that is missing prints `store.not_a_store`. | - | config |
| `config.no_such_run` | No run of the store has a key that starts with the prefix given to `inspect`. | - | config |
| `config.nothing_compared` | `diff` found no expectation that both evaluators graded in the same run, such as an evaluator version the store lacks. | - | config |
| `config.plugin_dir_no_manifest` | A directory of `plugins.dirs` does not exist or holds neither `plugin.toml` nor `candler-plugin.toml`. | - | config |
| `config.retired_setting` | `--set` names `request.treatment_source`, which `[treatment] source` in the experiment file replaced. | - | config |
| `config.run_directory_invalid` | The directory given to `inspect` holds a `manifest.json` but is not named by a run key, or is not below the `runs` directory of a store. | - | config |
| `config.toml_invalid` | A configuration file is not valid TOML. | - | config |
| `config.unknown_key` | A configuration file holds a key that no table defines, such as a misspelt setting. | The unknown key in the configuration file named in the message. | config |
| `config.wrong_type` | A configuration key holds a value of the wrong type. | The key in the configuration file named in the message. | config |
| `evaluation.artifact_glob_invalid` | An artifact glob in the needs of a kind is not a glob. | - | plugin |
| `evaluation.blob_corrupt` | A blob of the store does not have the digest the store records for it. | - | environment |
| `evaluation.cancelled` | The caller cancelled the request, for example by stopping the job; nothing is committed for it. | - | environment |
| `evaluation.context_io_failed` | A file system call failed while the run's files were materialised for an evaluator. | - | environment |
| `evaluation.dependency_cycle` | Entries of the suite depend on each other in a loop, so no order of requests serves them. | - | suite |
| `evaluation.dependency_unknown` | An entry of the suite depends on an id that no entry holds. | - | suite |
| `evaluation.evaluator_not_started` | The evaluator's command, as the plug-in manifest gives it, cannot be started. | - | plugin |
| `evaluation.evaluators_interleaved` | Entries of one evaluator depend on entries of another that depend on them in turn, so no order of requests serves them. | - | suite |
| `evaluation.input_unavailable` | A kind needs an input that the run does not hold, that is reserved for the image, or whose list is not in the store. | - | plugin |
| `evaluation.io_failed` | A file system call of the engine failed, such as creating the directory for the records. | - | environment |
| `evaluation.metadata_missing` | A kind needs a metadata path that the run manifest does not hold, or holds without a value. | - | plugin |
| `evaluation.need_unsupported` | The needs of a kind ask for something this version of Candler does not supply, such as snapshots. | - | plugin |
| `evaluation.run_not_sealed` | A run selected for evaluation has no `run.done` seal, so it can still change. | - | environment |
| `evaluation.run_selection_invalid` | The selection of runs is not a glob. | - | config |
| `evaluation.state_home_unset` | Neither `XDG_STATE_HOME` nor `HOME` is set, so the environments of plug-ins have no home. | - | environment |
| `evaluation.suite_changed` | The suite file of an evaluate job changed between the launcher reading it and the supervisor running the job, so the job refuses to grade with another suite. | - | suite |
| `evaluation.suite_unreadable` | The suite file the caller named does not exist or cannot be read. | - | config |
| `evaluation.workspace_unusable` | The workspace directory is relative, not valid UTF-8, inside the store or above it. | - | config |
| `harness.version_unreadable` | The output of the harness version probe names no version. | - | environment |
| `import.file_unreadable` | A file or directory of the legacy results folder cannot be read. | - | environment |
| `import.grading_repeated` | A grading file of a run is in two evaluation directories. | - | environment |
| `import.json_invalid` | A file of the legacy results folder is not valid JSON. | - | environment |
| `import.layout_invalid` | A file of the legacy results folder is not what the legacy format requires. | - | environment |
| `import.no_stream` | The path given to `ingest` holds no raw trace stream. | - | environment |
| `import.outputs_missing` | A run directory of the legacy results folder has no `outputs/` directory. | - | environment |
| `import.path_invalid` | A path found in the legacy results folder cannot be a path relative to a run. | - | environment |
| `import.record_unmappable` | A file of the legacy results folder does not map to a valid record. | - | environment |
| `import.rules_required` | The path given to `ingest` is a legacy results folder and no `--rules` file was given. | - | config |
| `import.run_differs` | A run that is finished in the store differs from the folder that is imported for it. | - | environment |
| `import.sources_disagree` | Two files of the legacy results folder give different values for one fact of a run. | - | environment |
| `import.status_unknown` | A status in the legacy results folder is not one the importer knows. | - | environment |
| `import.store_failed` | The store failed while a run of the legacy results folder was imported. | - | environment |
| `import.unreadable_path` | The path given to `ingest` cannot be read. | - | environment |
| `import.value_invalid` | A value found in the legacy results folder has the wrong form for what it names, such as a run name that is not a run key. | - | environment |
| `import.verdict_unknown` | A verdict in the legacy results folder is not `clean`, `incomplete` or `contaminated`. | - | environment |
| `isolation.claude_version_unreadable` | The version output of Claude Code in the image names no version. | - | environment |
| `isolation.docker_failed` | Docker exited with an error. | - | environment |
| `isolation.docker_resource_missing` | Docker has no such container, network or image. | - | environment |
| `isolation.docker_step_failed` | Docker failed while the run was being placed in a container. | - | environment |
| `isolation.docker_unavailable` | The docker client cannot be reached or does not offer the call. | - | environment |
| `isolation.env_name_invalid` | An environment name is empty or holds `=` or a NUL byte. | - | config |
| `isolation.env_name_repeated` | The environment sets one name twice. | - | config |
| `isolation.env_name_reserved` | The environment overlay sets a name that the wrapper owns. | - | config |
| `isolation.env_secret_refused` | The environment overlay carries a secret, which never enters the agent container. | - | config |
| `isolation.gateway_capped` | The concurrency cap of the sidecar answered with status 429. | - | environment |
| `isolation.gateway_misconfigured` | The configuration of the gateway is unusable. | - | environment |
| `isolation.gateway_no_answer` | The gateway started and nothing answered within the retries. | - | environment |
| `isolation.gateway_start_failed` | The gateway container did not start. | - | environment |
| `isolation.gateway_unauthorized` | The upstream refused the credential with status 401. | - | environment |
| `isolation.gateway_unavailable` | The gateway the run needs is not available. | - | environment |
| `isolation.gateway_upstream_error` | The upstream answered with an error status other than 401 and 429. | - | environment |
| `isolation.guard_docker_failed` | Docker failed under the container guard; nothing was destroyed after that call. | - | environment |
| `isolation.guard_job_unreadable` | The job directory could not be read, so the guard could not tell what is recorded. | - | environment |
| `isolation.image_build_failed` | An image build failed. | - | environment |
| `isolation.image_label_mismatch` | The binary in the image and the image's label name different Claude Code versions. | - | environment |
| `isolation.image_lock_mismatch` | The lock bytes do not have the digest the caller named. | - | environment |
| `isolation.image_service_unavailable` | The image service is not available. | - | environment |
| `isolation.mount_docker_socket` | A mount source names the docker socket or sits under a run directory. | - | config |
| `isolation.mount_outside_roots` | A mount source sits outside the project root and the store. | - | config |
| `isolation.mount_path_unsupported` | A mount path holds a comma, a newline or text that is not UTF-8, which cannot be written as a `--mount` option; a project directory with a comma in its name does it. | - | config |
| `isolation.mount_symlink_escape` | A mount source is a symlink that leaves the project root and the store. | - | config |
| `isolation.mount_unresolvable` | A mount source cannot be resolved on the host. | - | config |
| `isolation.secret_missing` | A secret the experiment declares is not set in the environment. | - | environment |
| `isolation.secret_unsupported_character` | A secret holds a character that the gateway's configuration cannot carry. | - | environment |
| `isolation.step_unavailable` | The container runtime has no isolation for the step, so the run cannot be placed. | - | environment |
| `isolation.sweep_list_failed` | Docker failed while the sweeper listed containers. | - | environment |
| `jobs.already_supervised` | Another supervisor already holds the lock of the job. | - | environment |
| `jobs.child_not_reaped` | A host process left behind by a lost job could not be reaped. | - | environment |
| `jobs.directory_exists` | The directory of a job to create already exists. | - | environment |
| `jobs.docker_unavailable` | The docker client cannot be reached when a job's session starts. | - | environment |
| `jobs.gateway_not_serving` | The gateway of the job does not serve, for example because it did not start or refused the token. | - | environment |
| `jobs.io_failed` | A file system call on the job's directory failed, such as a read, a write or a rename, or the store's `jobs` directory cannot be entered. | - | environment |
| `jobs.job_directory_failed` | The job's directory, its log or its state failed while the supervisor ran. | - | environment |
| `jobs.job_directory_unavailable` | The job's directory cannot be opened or read when its session starts. | - | environment |
| `jobs.job_held` | Another process holds the job, so this one cannot take it. | - | environment |
| `jobs.job_not_a_directory` | The path of a job in the store is not a directory. | - | environment |
| `jobs.job_not_found` | No directory of the store holds the job whose id was given. | - | config |
| `jobs.job_unreadable` | The files of a job's directory cannot be read. | - | environment |
| `jobs.recovery_store_failed` | The store could not be read or written while a lost job was recovered. | - | environment |
| `jobs.slot_io_failed` | A file system call on the slots directory failed, such as opening or locking a slot file. | - | environment |
| `jobs.supervisor_did_not_start` | The detached supervisor did not take the job's lock by the deadline, or exited first; its log says why. | - | environment |
| `jobs.supervisor_spawn_failed` | The supervisor process could not be started or given the job's spec. | - | environment |
| `jobs.token_secret_unreadable` | The secret that `model.token_secret` names cannot be read. | - | environment |
| `jobs.token_secret_unset` | The setting `model.token_secret` names no secret. | - | config |
| `jobs.treatment_unset` | The setting `request.treatment` is not set to a treatment. | - | config |
| `resolve.path_not_unicode` | A path that has to be written as a JSON string is not valid UTF-8. | - | config |
| `resolve.preflight_failed` | At least one preflight step failed; every step ran and the message lists the failures. | - | environment |
| `resolve.preflight_step_failed` | A preflight step failed before it could report, for example a missing directory or program. | - | environment |
| `resolve.value_invalid` | A value of the resolution has the wrong form for what it names, such as a name with a space. | - | config |
| `runner.catalog_command_failed` | The command that prints the tool catalog exited non-zero or was ended by a signal. | - | environment |
| `runner.catalog_timeout` | The command that prints the tool catalog overran its time. | - | environment |
| `runner.catalog_unreadable` | The output of the catalog command is not a tool catalog. | - | environment |
| `runner.census_malformed` | The init frame lacks a list the census needs, or the list holds an entry of the wrong kind. | - | environment |
| `runner.census_no_init` | The first session of a run has no init frame, so Claude Code did not run usably. | - | environment |
| `runner.experiment_locked` | Another supervisor holds the experiment, or a job that owns an unfinished attempt is alive. | - | environment |
| `runner.fact_changed` | A fact re-checked now differs from the stored cell and the plan was not forced; `--force` accepts the difference. | - | environment |
| `runner.stored_cell_unknown` | A past experiment has stored cells that no cell of this experiment answers, so it cannot be replayed. | - | experiment |
| `store.digest_mismatch` | Bytes read from or written to the store do not have the digest that was expected of them. | - | environment |
| `store.io_failed` | A file system call of the store failed, such as a read, a write or a rename. | - | environment |
| `store.not_a_store` | The directory named as the store has no valid `store.json` marker. | - | environment |
| `store.run_exists` | A run directory already exists and the caller did not ask to replace it, or a finished run cannot be replaced. | - | environment |
| `store.run_sealed` | The run is sealed by `run.done` and cannot change. | - | environment |
| `store.value_invalid` | A value read from or written to the store has the wrong form for what it names, such as a run key that is not one. | - | environment |
| `supervise.child_survived_kill` | A child process is still running after it was sent `SIGKILL`. | - | environment |
| `supervise.io_failed` | A file or process operation of the supervisor failed. | - | environment |
| `supervise.spawn_failed` | The operating system refused to start the program, such as a program that is not installed. | - | environment |
<!-- /generated:refusals -->
