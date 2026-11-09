# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
