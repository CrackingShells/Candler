# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.0](https://github.com/CrackingShells/Candler/compare/v0.15.0...v0.16.0) - 2026-11-25

### Documentation

- *(api)* render the spec rows without a table
- *(api)* name the cap spec entries as items
- *(api)* show every setting of a path and one cell wording
- *(api)* generate pages from the declaration
- *(library)* say where effort and mode text is checked and mapped
- *(library)* point the name check at the published pattern
- *(library)* say what the conformance collector is in the driver tests

### Features

- *(runner)* store the tool-catalog snapshot when the run is sealed
- *(resolve)* [**breaking**] read replicates once and refuse the silent inputs left
- *(resolve)* [**breaking**] read MCP servers in one place and refuse a malformed one

### Fixes

- *(importer)* keep an empty snapshots list in a raw-ingested cell
- *(library)* attribute MCP calls the same way on both harnesses
- *(library)* ask nothing of a driver for a mode left unset
- *(runner)* record each output's execute bit when a run is sealed
- *(resolve)* read the replicates count in one place
- *(cli)* render the failures of every treatment level alike
- *(cli)* resolve the kill targets before asking for confirmation
- *(resolve)* check each cell against its own level
- *(cli)* tell an agent off a terminal how to confirm a kill
- *(cli)* refuse kill without --yes off a terminal
- *(library)* refuse unknown effort and mode text on a stored cell
- *(importer)* write an imported cell's snapshots as a list, even empty
- *(library)* render MCP servers from their type in both drivers
- *(library)* ask the driver about the mode of a cell that names none
- *(library)* read every permission mode Claude Code reads
- *(experiment)* point an MCP server refusal at its field
- *(library)* accept only server names no harness rewrites or splits
- *(library)* state each setting's path once and sweep the removed names

### Refactoring

- *(orchestrator)* drop snapshots from the typed roots of a cell
- *(runner)* remove the constant catalog producer, show MCP config
- *(library)* state every per-harness fact in one shape
- *(resolve)* [**breaking**] refuse the snapshots path in an experiment
- *(library)* state each setting's path once, parse at resolve
- *(library)* read a cell's settings once and refuse unknown text
- *(library)* read MCP servers through the library in both drivers

## [0.15.0](https://github.com/CrackingShells/Candler/compare/v0.14.0...v0.15.0) - 2026-11-24

### Features

- *(resolve)* [**breaking**] refuse inputs that were silently accepted

### Fixes

- *(runner)* keep workspace/ after the seal for its test readers
- *(adapter-opencode)* collect the files an OpenCode run wrote
- *(runner)* store each output's bytes as a blob when the run is sealed

### Refactoring

- *(runner)* remove the unused plan summary and duplicate mappers

## [0.14.0](https://github.com/CrackingShells/Candler/compare/v0.13.0...v0.14.0) - 2026-11-23

### Documentation

- *(cli)* generate the configuration file page from the reader's keys
- *(isolation)* keep the agent path comment on its constant

### Features

- *(resolve)* refuse reserved names in the experiment env table
- *(cli)* [**breaking**] print the analysis report record and a run's transcript
- *(cli)* print exit codes in help from one constant
- *(resolve)* read the configuration paths from one table
- *(cli)* [**breaking**] print plan counts, and records for show and ingest
- *(cli)* [**breaking**] print refusals as json by default and retire --error-format
- *(cli)* [**breaking**] print every data verb's record as json by default
- *(resolve)* refuse a pinned commit that lacks the skill tree
- *(cli)* print the summary and the spend in analyze and inspect
- *(resolve)* refuse names in the env table that are not variable names
- *(isolation)* pass the experiment env table to the harness
- *(jobs)* probe a server in a container staged as a run's
- *(jobs)* refuse a job whose declared server exits at start
- *(runner)* bound a plan by its caps
- *(runner)* count the cells and runs of a plan
- *(analysis)* derive a decision summary and a spend from the report

### Fixes

- *(resolve)* refuse env given outside the env table
- *(analysis)* fail on an event that cannot be rendered
- *(resolve)* keep a misspelt per-run cap out of the known paths
- *(cli)* say when the no-store refusal is raised
- *(cli)* render show and ls text from their record
- *(cli)* withdraw the resolve plan counts
- *(cli)* drop retired-flag prose and history wording
- *(cli)* name the explicit store root in what resolve prints
- *(resolve)* check a replicates value of the file and of --set once
- *(resolve)* refuse a replicates setting that is not a count
- *(resolve)* word the reserved env row without a name list
- *(resolve)* drop an unused import from the path table test
- *(jobs)* code the server start refusals
- *(jobs)* drop an unused import from the start check
- *(jobs)* give a probed server its env apart from the container's
- *(runner)* bound a plan by the wall cap its runs get
- *(runner)* take the plan caps from the experiment file
- *(resolve)* keep the cause in a skill tree refusal
- *(runner)* honour replicates set on the command line
- *(analysis)* reflow the level order doc comment
- *(analysis)* break level ties by JSON text and pin decision fields
- *(analysis)* order compared levels by their values, not cell digests

### Refactoring

- *(analysis)* name the comparability class apart from a block
- *(runner)* extract the staging of a cell's files from drive
- *(runner)* extract the cell container composition from drive

## [0.13.0](https://github.com/CrackingShells/Candler/compare/v0.12.5...v0.13.0) - 2026-11-18

### Documentation

- *(analysis)* record the weighted known-truth evidence

### Features

- *(analysis)* report every difference as B minus A
- *(analysis)* report blocked comparisons in analyze
- *(runner)* admit a table of treatment fields as a treatment level
- *(jobs)* run each cell with its own treatment level
- *(runner)* admit a treatment factor in the CLI preflight
- *(analysis)* report instrument errors per arm and keep per-run units
- *(analysis)* compare declared factors blocked by the others
- *(analysis)* build blocks and choose each comparison's test
- *(runner)* plan each cell with its own treatment level
- *(analysis)* let a treatment factor explain lock and image changes
- *(runner)* add a seeded run order
- *(runner)* admit the treatment as a declared factor
- *(stats)* compute the CMH test and the MH odds ratio
- *(stats)* add the stratified BCa interval
- *(stats)* add the stratified permutation test
- *(stats)* add a seeded SplitMix64 with a Python twin
- *(stats)* compute the mid-p and the exact conditional test

### Fixes

- *(importer)* declare the treatment as an imported factor
- *(analysis)* count treatment pairs; a flat bootstrap is inconclusive
- *(jobs)* apply a one-level treatment factor and code plan refusals
- *(analysis)* drop the unreachable prefix branch of level_of
- *(resolve)* key the treatment verdict on request.treatment
- *(analysis)* reject only tested entries and key weighted blocks
- *(runner)* pin the in-flight pair and the seed contract
- *(stats)* hold the Mantel-Fleiss tie and guard the level and OR 1
- *(stats)* pin the tie tolerance and the BCa acceleration to engines

### Refactoring

- *(runner)* plan runs in the seeded order

## [0.12.5](https://github.com/CrackingShells/Candler/compare/v0.12.4...v0.12.5) - 2026-11-17

### Refactoring

- *(library)* tag images by the build context digest
- *(library)* walk and digest trees through one tree module

## [0.12.4](https://github.com/CrackingShells/Candler/compare/v0.12.3...v0.12.4) - 2026-11-17

### Refactoring

- *(isolation)* check prune, package, host names via primitives
- *(runner)* fold plan names through FieldPath::segment_of
- *(library)* check session ids and env names via primitives
- *(adapter-opencode)* take reason codes and ids from primitives

## [0.12.3](https://github.com/CrackingShells/Candler/compare/v0.12.2...v0.12.3) - 2026-11-17

### Refactoring

- *(library)* keep the layering guard's words out of a doc comment
- *(library)* name the cell-path error at each reader
- *(resolve)* read a context value by text through ResolveContext
- *(library)* read a cell by text and as a table through Cell
- *(adapter-claude-code)* name the cell-path error at each reader
- *(adapter-opencode)* name the cell-path error at each reader

## [0.12.2](https://github.com/CrackingShells/Candler/compare/v0.12.1...v0.12.2) - 2026-11-17

### Refactoring

- *(runner)* run git and probes through one helper set

## [0.12.1](https://github.com/CrackingShells/Candler/compare/v0.12.0...v0.12.1) - 2026-11-17

### Refactoring

- *(runner)* check duplicates through the schemas helper

## [0.12.0](https://github.com/CrackingShells/Candler/compare/v0.11.0...v0.12.0) - 2026-11-17

### Features

- *(adapter-claude-code)* give an MCP server its production name

## [0.11.0](https://github.com/CrackingShells/Candler/compare/v0.10.1...v0.11.0) - 2026-11-17

### Documentation

- *(isolation)* read treatment.source as the checkout's root

### Features

- *(isolation)* build an extension from the treatment's tree

## [0.10.1](https://github.com/CrackingShells/Candler/compare/v0.10.0...v0.10.1) - 2026-11-17

### Refactoring

- *(experiment)* delete the test-only Surface type

## [0.10.0](https://github.com/CrackingShells/Candler/compare/v0.9.0...v0.10.0) - 2026-11-17

### Features

- *(isolation)* enforce isolation.egress and lift its refusal
- *(isolation)* trust a configured CA for the model upstream
- *(isolation)* forward listed hosts and ports through the egress proxy
- *(isolation)* refuse ground truth in what the agent receives
- *(isolation)* plan one egress proxy per allowlist

### Fixes

- *(isolation)* give the supervisor the launcher's config home
- *(isolation)* forward egress bytes through njs buffer events
- *(isolation)* drop surface's unread mcp field
- *(isolation)* use a relative server path in the agent-env fixture
- *(isolation)* give internal networks no host address
- *(isolation)* stop the agent once and list once at destroy
- *(isolation)* destroy a container that was created but never ran
- *(isolation)* tear down a job's allow networks
- *(runner)* render a plug-in's mcp servers for claude code

### Performance

- *(isolation)* verify a destroy with one inspect of every container

## [0.9.0](https://github.com/CrackingShells/Candler/compare/v0.8.5...v0.9.0) - 2026-11-16

### Features

- *(isolation)* append a plug-in environment's lists and inputs
- *(isolation)* refuse egress until the proxy lands
- *(isolation)* provision a plug-in's agent environment as a layer
- *(isolation)* resolve an egress allowlist of hostnames and ports
- *(isolation)* inspect many containers in one call and create by id
- *(isolation)* admit the egress and plug-in environment paths

### Fixes

- *(isolation)* destroy the agent container when its client cannot start
- *(isolation)* refuse a lock split between plug-in and experiment
- *(isolation)* stop the agent container through the daemon

### Refactoring

- *(isolation)* create the agent container before attaching to it
- *(isolation)* build every docker process in one client

### Style

- *(isolation)* add the missing Grit annotations

## [0.8.5](https://github.com/CrackingShells/Candler/compare/v0.8.4...v0.8.5) - 2026-11-13

### Refactoring

- *(library)* plan each run through one driver call

## [0.8.4](https://github.com/CrackingShells/Candler/compare/v0.8.3...v0.8.4) - 2026-11-13

### Refactoring

- *(isolation)* take the base image's harness version from a driver
- *(library)* read cells through one set of paths and accessors
- *(library)* build the OpenCode interrupt with no error to drop

## [0.8.3](https://github.com/CrackingShells/Candler/compare/v0.8.2...v0.8.3) - 2026-11-13

### Documentation

- *(contributing)* name tests by module path in the bless commands

### Refactoring

- *(library)* let the harness trait own what the CLI held

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
