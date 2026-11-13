# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.2](https://github.com/CrackingShells/Candler/compare/v0.8.1...v0.8.2) - 2026-11-13

### Fixes

- *(analysis)* count each run's expectation once across regrades

### Performance

- *(runner)* keep one copy of the trace and seal off the async workers
- *(runner)* read the store once per plan

### Refactoring

- *(run-store)* build digests with the schemas' constructors
- *(resolve)* take every value path from experiment::paths
- *(run-store)* delete the store index nobody reads

## [0.8.1](https://github.com/CrackingShells/Candler/compare/v0.8.0...v0.8.1) - 2026-11-13

### Fixes

- *(isolation)* key the env image on the env Dockerfile too
- *(jobs)* take a lost job's lock off the async worker
- *(isolation)* read a run's gateway signals from its own attempt

### Performance

- *(runner)* read the boot id once per process
- *(jobs)* queue slot waiters on a semaphore, not on the files
- *(isolation)* give only the agent container a copy of the env
- *(runner)* remove a child's record once its wait ends it
- *(jobs)* read a job's end from the back of its event log

## [0.8.0](https://github.com/CrackingShells/Candler/compare/v0.7.0...v0.8.0) - 2026-11-13

### Documentation

- *(runner)* add the harness capability page
- *(runner)* add the driver-traits contract page
- *(runner)* say what a collect command sees
- *(runner)* say hooks_incomplete names the instrument
- *(adapter-opencode)* quote the harness name in the crate doc

### Features

- *(runner)* refuse session mode on a harness whose controls fail
- *(adapter-opencode)* register the driver in the binary
- *(experiment)* opt a cell into session mode
- *(cli)* [**breaking**] select the harness driver by the experiment's name
- *(runner)* collect a harness's export and read it offline
- *(runner)* close a session only once the agent has yielded
- *(isolation)* keep a session's stdin open
- *(manifest)* give imported strata a verdict on their recorded build
- *(runner)* declare the settings table a driver answers
- *(runner)* refuse a setting a harness cannot honour, naming its path
- *(runner)* code the census inventory refusals
- *(runner)* code the session-record and session-id refusals
- *(runner)* let a driver supply gateway, state, census and session
- *(adapter-claude-code)* add session mode and the new driver facts
- *(adapter-opencode)* map the settings opencode can honour
- *(adapter-opencode)* refuse unsupported settings with their path
- *(adapter-opencode)* drive opencode v2 through the shared traits

### Fixes

- *(runner)* refuse an unsupported setting when the cells are planned
- *(runner)* read a run's census and export the way the run did
- *(experiment)* settle the session rule for every cell
- *(isolation)* [**breaking**] retire the claude-named version refusal code
- *(isolation)* give the version probe the home a run gives the harness
- *(isolation)* key the env image by its base as well as its lock
- *(runner)* name the fake model's host without a harness name
- *(adapter-claude-code)* cover Stop lines by order and prompt id
- *(runner)* charge a lost Stop line to the instrument
- *(runner)* record session totals and a silent hook channel
- *(adapter-opencode)* refuse an unknown effort level as unsupported
- *(adapter-claude-code)* tie each Stop line to its turn
- *(adapter-claude-code)* need a Stop line for every turn
- *(adapter-opencode)* refuse dated 3.x ids, haiku and ultracode effort
- *(adapter-opencode)* map only what OpenCode honours like Claude Code

### Refactoring

- *(runner)* consolidate opencode_wiring
- *(cli)* recognise raw trace streams through the registry
- *(isolation)* read the harness version through its driver
- *(isolation)* let a caller find or build the base image
- *(runner)* consolidate session_runner
- *(runner)* name the session mode, mark dropped errors
- *(isolation)* harness-neutral version field, one agent home
- *(runner)* take the run totals from the driver's live reader
- *(isolation)* take the gateway, state and census from the driver
- *(runner)* frame raw imports with the reader's framing type
- *(runner)* [**breaking**] frame streams with the reader's own framing type

## [0.7.0](https://github.com/CrackingShells/Candler/compare/v0.6.1...v0.7.0) - 2026-11-12

### Documentation

- *(cli)* say that a missing store root is not raised
- *(contributing)* name the variants, the cli codes and tried notes
- *(contributing)* list the refusals of each contract page
- *(contributing)* generate the contract pages for six surfaces
- *(evaluation)* state the shape of an equivalence file in its refusal

### Features

- *(cli)* state why each option is not derived
- *(cli)* infer the evaluator and take a bare version
- *(cli)* embed the image assets and cache them by digest
- *(cli)* print the refusal a failed evaluate job recorded
- *(evaluation)* give an evaluate job the caller's environment
- *(cli)* end evaluate at once on a second Ctrl-C
- *(cli)* [**breaking**] start evaluate as a job and follow it to its report
- *(cli)* supervise an evaluate job from the hidden verb
- *(cli)* code the refusals of import and ingest
- *(cli)* code the refusals of jobs and evaluation
- *(cli)* add inspect to show a run with its evidence
- *(cli)* ingest raw trace directories and results folders
- *(cli)* carry the code of an analyze refusal to the caller
- *(cli)* add analyze with JSON and CSV output
- *(cli)* show the suite of an evaluate job in ps
- *(cli)* show the experiment or the suite of each job in ps
- *(cli)* default watch to json lines and add a live view
- *(cli)* add ls to list the experiments of a project
- *(cli)* add resolve to print every value and its source
- *(cli)* print a refusal as refusal/1 under --error-format json
- *(jobs)* record the refusal a failed evaluate job ended with
- *(jobs)* code the refusal of a suite changed after the job started
- *(importer)* ingest raw claude code trace directories
- *(analysis)* choose one evaluator version and compare the pairs
- *(jobs)* supervise an evaluation as a job
- *(evaluation)* take evaluator slots and report committed outcomes
- *(evaluation)* add the evaluator slot pool and plug-in discovery
- *(cli)* code the refusals of the core and the harnesses
- *(analysis)* tally outcomes into arm rates with their counts
- *(stats)* add Wilson, Newcombe, Fisher and Holm methods

### Fixes

- *(cli)* code the rules-file refusals of ingest
- *(cli)* register the cause carriers of kill and ps
- *(cli)* carry the failing job's error without new error enums
- *(cli)* code the refusals a caller causes in each verb
- *(cli)* code the early refusals of evaluate and print a job as written
- *(jobs)* recover a lost evaluate job without a docker client
- *(cli)* reap the supervisor and follow a job that ends at once
- *(cli)* code the rules file that does not parse
- *(cli)* keep the codes of the rules and equivalence refusals
- *(importer)* take a results folder by its root markers
- *(cli)* map job-open failures by kind and align wrapper answers
- *(cli)* print only a code with the side its registry row gives
- *(cli)* keep the wrapper's side for a value check and name the cause
- *(importer)* refuse a results folder given without rules
- *(analysis)* tally arms per stratum and read factors from manifests
- *(cli)* [**breaking**] refuse a field path that no path class knows
- *(cli)* refuse a factor over a setting before any image work
- *(cli)* [**breaking**] refuse a --set key under request that no setting names
- *(jobs)* see a job that ends at once as started
- *(cli)* answer a wrapped refusal with the side of its cause

### Refactoring

- *(cli)* embed the assets by a path relative to the source
- *(cli)* rename the import command file to ingest
- *(cli)* build the evaluate request from the new engine fields

### Style

- *(cli)* write the analyze option stamps in lower case

## [0.6.1](https://github.com/CrackingShells/Candler/compare/v0.6.0...v0.6.1) - 2026-11-11

### Fixes

- *(jobs)* sweep orphans that start in the supervisor's tick
- *(jobs)* read a process's parent from two sources that agree

### Refactoring

- *(cli)* scaffold the lean cli verbs
- *(cli)* move the foreground preflight out of run
- *(jobs)* add the job spec file enum

## [0.6.0](https://github.com/CrackingShells/Candler/compare/v0.5.0...v0.6.0) - 2026-11-11

### Features

- *(experiment)* [**breaking**] replace the treatment source setting with the file's
- *(experiment)* read the treatment's source from the experiment file
- *(experiment)* hash and store the inputs an experiment declares
- *(isolation)* [**breaking**] record the packages an experiment names, not a planner
- *(importer)* [**breaking**] take the imported entry's params from the rules
- *(runner)* mount agent inputs and record evaluator-only inputs
- *(evaluation)* serve an evaluator's input needs from the run
- *(manifest)* gate a stratum on the build's commit, not its tag
- *(isolation)* [**breaking**] take the agent environment from the experiment

### Fixes

- *(manifest)* record only the build identity the build can prove
- *(manifest)* record a build's identity when it is not a git checkout
- *(experiment)* keep settings written in the file out of the cell
- *(manifest)* count evaluation_inputs among the nuisance paths

## [0.5.0](https://github.com/CrackingShells/Candler/compare/v0.4.1...v0.5.0) - 2026-11-10

### Features

- *(isolation)* write an agent container's record from its cidfile

### Fixes

- *(runner)* record the agent container while its run is live
- *(importer)* [**breaking**] list each run's stream and stderr as import sources

### Refactoring

- *(repo)* [**breaking**] deny bare allow attributes and settle every suppression
- *(adapter-claude-code)* [**breaking**] build the driver's id and probe once
- *(resolve)* [**breaking**] borrow the services the registration steps clone

## [0.4.1](https://github.com/CrackingShells/Candler/compare/v0.4.0...v0.4.1) - 2026-11-09

### Refactoring

- *(jobs)* share one recovery of a lost job between cli and runtime

## [0.4.0](https://github.com/CrackingShells/Candler/compare/v0.3.0...v0.4.0) - 2026-11-09

### Features

- *(cli)* wire candler show and diff to the core
- *(cli)* share one exit mapping and wire import and evaluate
- *(cli)* take a suite file, select an evaluator and share helpers
- *(cli)* scaffold the import, evaluate, show and diff commands
- *(evaluation)* report the deadline each request was given
- *(evaluation)* run requests concurrently and commit outcome files
- *(evaluation)* plan items, applies_to and dependencies per run
- *(importer)* record the rules digest and refuse a changed one
- *(evaluator-protocol)* run evaluator requests through the supervisor
- *(evaluation)* materialise contexts into private workspaces
- *(evaluation)* assemble run contexts from suite needs
- *(evaluation)* diff evaluator versions on the four outcome fields
- *(evaluation)* compute quality and gate rates per cell
- *(evaluation)* select current outcomes and flag i4 changes
- *(registry)* build plug-in environments once per lock digest
- *(registry)* check suites against plug-in manifests
- *(evaluation)* pass the supervisor, cancel and env root to evaluate
- *(evaluation)* scaffold the evaluation modules and the evaluate seam

### Fixes

- *(evaluation)* run a uv plug-in inside its environment
- *(evaluation)* read a suite file written as json
- *(evaluation)* refuse a workspace that contains the store
- *(evaluation)* gate the unix-only surface anchors and doc links

### Performance

- *(run-store)* sync with fsync and leave scratch copies unsynced
- *(evaluation)* send one request per run, evaluator and stage

### Refactoring

- *(cli)* share one store root across every verb

## [0.3.0](https://github.com/CrackingShells/Candler/compare/v0.2.0...v0.3.0) - 2026-11-09

### Documentation

- *(isolation)* complete the guard lines of B32, B34 and B36

### Features

- *(runner)* record the reference provenance fields
- *(cli)* start a job detached and print its id
- *(cli)* resolve and plan a run from any directory
- *(cli)* compose the M2b and M2a preflight steps
- *(jobs)* leave a starting job alone and bound the whole teardown
- *(jobs)* hold the job lock as a detached subreaper
- *(cli)* follow a job's events with watch
- *(cli)* stop jobs with a confirmation and a guarded reap
- *(cli)* list jobs with their state and progress
- *(cli)* scaffold the run, ps, kill and watch verbs
- *(jobs)* give the launcher an id, a digest and an identity
- *(jobs)* end a job killed on SIGTERM
- *(jobs)* sweep strays by marker and by reparenting
- *(jobs)* stop live runs when a job cap crosses
- *(jobs)* run a job's plans under the parallel cap
- *(jobs)* start the job gateway on its own network
- *(jobs)* hand the supervisor its job, services, force and inputs
- *(runner)* mount a cell's inputs and name the manifest error
- *(runner)* wire exec_run through the harness driver and the store
- *(isolation)* set a run's outputs and collect it through the guard
- *(runner)* seal a run with run.done last
- *(runner)* retry a gateway failure once and collect after exit
- *(runner)* classify the end of a run in the R2 order
- *(runner)* spawn a run in its container with a live observer
- *(runner)* record forced deviations on every new attempt
- *(runner)* replay the stored cell and refuse changed facts
- *(runner)* seal unfinished attempts and number the next one
- *(runner)* number runs and take the experiment lock
- *(isolation)* start the gateway with the token on stdin
- *(catalog)* refuse a catalog that holds an absolute path
- *(catalog)* run the catalog command in a control container
- *(isolation)* wrap agent commands in a self-expiring timeout
- *(isolation)* build the hardened docker run argv
- *(isolation)* build container mounts only from canonical roots
- *(isolation)* run image control calls through the docker client
- *(isolation)* build the pinned env image once per lock digest
- *(isolation)* read the Claude Code version from the image
- *(isolation)* sweep labelled containers from the stop hook
- *(isolation)* destroy containers only through one verified guard
- *(isolation)* run the docker cli through the supervisor
- *(runner)* refuse input leaks and build the run tree
- *(census)* measure the tools and servers the agent saw
- *(catalog)* capture the catalog through a control container
- *(manifest)* give the stratum verdict over declared factors
- *(manifest)* diff comparable values by path segment
- *(jobs)* share run slots across jobs by flock
- *(jobs)* classify a job as running, ended or lost
- *(jobs)* append and tail the job event log
- *(jobs)* write the job directory and read its children
- *(resolve)* register the m2b preflight steps
- *(resolve)* derive the image id and the build input digest
- *(resolve)* derive the harness and treatment identities
- *(experiment)* read the configuration surface and split the budget
- *(isolation)* keep the destroyed containers' labels in a sweep
- *(isolation)* record the gateway sidecar before it holds the token
- *(isolation)* give each job an egress network for bridge cells
- *(isolation)* compile the env lock and build an experiment layer
- *(isolation)* tag image builds and place the env and control dirs
- *(orchestrator)* scaffold the M2b modules, traits and types
- *(adapter-claude-code)* classify exits and collect outputs
- *(adapter-claude-code)* prepare settings, mcp config, skill tree
- *(adapter-claude-code)* build the reference command line

### Fixes

- *(cli)* match the binary's tag and watch what makes it dirty
- *(resolve)* resolve experiment-file paths against the file
- *(adapter-claude-code)* archive only the treatment's skill subtree
- *(cli)* map every verb to one exit enum and exit 2 on error
- *(orchestrator)* settle the isolation and runtime seams from review
- *(isolation)* name the sandbox contract candler-container@1
- *(runner)* leave skill_version unrecorded for a foreign tag
- *(runner)* emit container.created and measure the run's end
- *(cli)* name the stored and observed values in a resume refusal
- *(resolve)* name the failed preflight steps
- *(orchestrator)* log why a run failed
- *(orchestrator)* stop a spawned supervisor when its guard fails
- *(orchestrator)* let a lost job's own view recover it
- *(jobs)* seed a run that enters after a stop with the stop's cause
- *(jobs)* give each cell its own sources
- *(isolation)* scope a run's destroy to the run and pre-make mounts
- *(runner)* refuse root names that map to one deviation path
- *(runner)* check input and environment digests as resume facts
- *(runner)* match stored cells per experiment, fail closed on liveness
- *(jobs)* write a slot holder as an object with no null
- *(isolation)* pass the token secret's name to Gateway::start

### Refactoring

- *(orchestrator)* share the gateway e2e helpers
- *(orchestrator)* let tests build the scaffold data structs

### Style

- *(runner)* annotate the borrows, indexes and loops the review found
- *(cli)* tag the trait objects and borrows of the kill command
- *(cli)* tag borrows and trait objects in ps, watch and kill guard
- *(runner)* drop the milestone words left in supervise and paths
- *(runner)* tag borrows in the plan and the signal guard
- *(jobs)* tag trait objects, borrows and no-op arms in the job code
- *(isolation)* tag the trait objects of the gateway
- *(runner)* tag borrows, trait objects and indexes in L1 code
- *(isolation)* note borrows and indexes, list every docker error
- *(runner)* note the borrow of the tool name in the census
- *(manifest)* put the oracle on its own doc line, note a borrow
- *(jobs)* note the borrows of the job store and its tests
- *(resolve)* note a borrow, wrap the oracle, drop a milestone name
- *(adapter-claude-code)* name B21 in the guard, note the borrows

## [0.2.0](https://github.com/CrackingShells/Candler/compare/v0.1.0...v0.2.0) - 2026-11-05

### Documentation

- *(runner)* state the resolution of a Linux start time
- *(readme)* describe the m1 crates, fixtures and schema roots
- *(runner)* mark test seams as test-only and finish the failed path
- *(runner)* state the accepted deviations and mark the index sites

### Features

- *(importer)* assemble runs idempotently and report the import
- *(importer)* import artifacts and their workspace tree
- *(importer)* frame legacy streams and normalise their traces
- *(runner)* reap a process group from its child record
- *(runner)* stop children by escalation and enforce deadlines
- *(runner)* spawn children in their own group with a record
- *(runner)* write and read framed raw streams
- *(importer)* map legacy grading to outcome records
- *(importer)* map legacy runs to manifests and integrity
- *(importer)* read a legacy results folder into a typed model
- *(run-store)* keep a rebuildable index of runs and outcomes
- *(run-store)* write one outcome file per request atomically
- *(run-store)* write runs with their tree, raw and trace
- *(run-store)* lay out the store and write blobs once
- *(resolve)* run registered preflight steps in order
- *(resolve)* layer resolved values with their sources
- *(resolve)* load the project and user config
- *(runner)* define the harness driver and trace reader traits
- *(runner)* define commands, stop specs and collect specs
- *(adapter-claude-code)* normalise sessions, subagents and denials
- *(adapter-claude-code)* detect and parse stream-json lines

### Fixes

- *(runner)* refuse a record whose group the process does not lead
- *(runner)* end what a stop leaves behind and bound what it waits for
- *(importer)* annotate every borrow in the legacy mapping

### Refactoring

- *(importer)* name the legacy format keys once
- *(importer)* take the resolved-version key from the rules
- *(run-store)* name the manifest mismatch text detail
- *(adapter-claude-code)* build the reader with a fallible new

### Style

- *(adapter-claude-code)* word the borrow notes on as_str plainly
- *(adapter-claude-code)* annotate borrows, dyn types and the ratchet
