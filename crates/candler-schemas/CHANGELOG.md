# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.15.0](https://github.com/CrackingShells/Candler/compare/schemas@0.14.0...schemas@0.15.0) - 2026-11-25

### Documentation

- *(schemas)* say an artifact glob starts with outputs/
- *(schemas)* say what a request is served and where a path is relative

### Features

- *(schemas)* [**breaking**] carry a shell call's exit and the stored run's needs

### Fixes

- *(schemas)* describe only what the wire carries in the needs docs
- *(schemas)* read an unknown key in needs as its siblings do

## [0.14.0](https://github.com/CrackingShells/Candler/compare/schemas@0.13.0...schemas@0.14.0) - 2026-11-25

### Documentation

- *(schemas)* say what context_start holds

### Features

- *(schemas)* one document fault for a bad level and a bad file
- *(schemas)* hold the free-key fact in one list
- *(schemas)* keep the keys of the hooks and global instruction tables
- *(schemas)* show the instructions and hooks of an inspected run
- *(schemas)* type the instructions and hooks of a cell
- *(schemas)* add the context_start trace event

## [0.13.0](https://github.com/CrackingShells/Candler/compare/schemas@0.12.0...schemas@0.13.0) - 2026-11-25

### Documentation

- *(schemas)* say how each harness reads an MCP origin

### Features

- *(schemas)* [**breaking**] drop the catalog snapshot and show MCP in inspect
- *(schemas)* name the tool of an MCP origin
- *(schemas)* add the snapshot's stored text and inspect's catalog
- *(schemas)* [**breaking**] type the MCP servers an experiment declares

### Fixes

- *(schemas)* say who writes snapshots and drop the unused path constant
- *(schemas)* keep an absent snapshots list apart from an empty one
- *(schemas)* [**breaking**] refuse a server name the harnesses would rewrite or split
- *(schemas)* default the codemode of an MCP server to false
- *(schemas)* give a replicates value that is no count its own code
- *(schemas)* say which type marker goes with each MCP server field
- *(schemas)* refuse in the MCP server schema what its reader refuses
- *(schemas)* refuse an unknown key in an MCP server entry

### Refactoring

- *(schemas)* answer every MCP transport in each driver

## [0.12.0](https://github.com/CrackingShells/Candler/compare/schemas@0.11.3...schemas@0.12.0) - 2026-11-23

### Documentation

- *(schemas)* list the analysis report schema file
- *(schemas)* name the refusal output without the retired flag

### Features

- *(schemas)* name the env path of the harness environment
- *(schemas)* add an env table to the experiment file
- *(schemas)* add the analysis report record
- *(schemas)* add the show counts, ingest and plan records
- *(schemas)* add the inspect and listing output records

### Fixes

- *(schemas)* drop the plan counts record
- *(schemas)* carry both sides of a diff change
- *(schemas)* let a treatment factor stand without a treatment table
- *(schemas)* report a JSON suite entry fault as a JSON fault
- *(schemas)* name the entry of a refused input or suite entry
- *(schemas)* [**breaking**] print the inner code for a value-invalid file

## [0.11.3](https://github.com/CrackingShells/Candler/compare/schemas@0.11.2...schemas@0.11.3) - 2026-11-17

### Refactoring

- *(schemas)* own the name grammars in primitives

## [0.11.2](https://github.com/CrackingShells/Candler/compare/schemas@0.11.1...schemas@0.11.2) - 2026-11-17

### Refactoring

- *(schemas)* name what lies below a prefix once on FieldPath

## [0.11.1](https://github.com/CrackingShells/Candler/compare/schemas@0.11.0...schemas@0.11.1) - 2026-11-17

### Documentation

- *(isolation)* read treatment.source as the checkout's root

### Refactoring

- *(schemas)* check duplicates through one helper

## [0.11.0](https://github.com/CrackingShells/Candler/compare/schemas@0.10.0...schemas@0.11.0) - 2026-11-16

### Features

- *(schemas)* declare agent environments and an egress allowlist

## [0.10.0](https://github.com/CrackingShells/Candler/compare/schemas@0.9.0...schemas@0.10.0) - 2026-11-13

### Features

- *(schemas)* own the cell paths, checked at compile time

## [0.9.0](https://github.com/CrackingShells/Candler/compare/schemas@0.8.0...schemas@0.9.0) - 2026-11-13

### Features

- *(schemas)* name literals checked at compile time, one segment test

## [0.8.0](https://github.com/CrackingShells/Candler/compare/schemas@0.7.1...schemas@0.8.0) - 2026-11-13

### Features

- *(schemas)* build a digest from bytes, a reader or a hasher

## [0.7.1](https://github.com/CrackingShells/Candler/compare/schemas@0.7.0...schemas@0.7.1) - 2026-11-13

### Refactoring

- *(schemas)* name the session's parent field without the stutter

## [0.7.0](https://github.com/CrackingShells/Candler/compare/schemas@0.6.0...schemas@0.7.0) - 2026-11-12

### Documentation

- *(schemas)* name the item at fault in a suite json refusal
- *(schemas)* correct two refusal rows of the outcome and experiment

### Features

- *(schemas)* [**breaking**] add the refusal record and refuse unknown keys

## [0.6.0](https://github.com/CrackingShells/Candler/compare/schemas@0.5.0...schemas@0.6.0) - 2026-11-11

### Features

- *(schemas)* declare inputs, table values and a treatment source

## [0.5.0](https://github.com/CrackingShells/Candler/compare/schemas@0.4.1...schemas@0.5.0) - 2026-11-10

### Refactoring

- *(repo)* [**breaking**] deny bare allow attributes and settle every suppression
- *(schemas)* avoid or justify each lint the wire types hit

## [0.4.1](https://github.com/CrackingShells/Candler/compare/schemas@0.4.0...schemas@0.4.1) - 2026-11-09

### Fixes

- *(schemas)* refuse a catalog server or snapshot written as a list

## [0.4.0](https://github.com/CrackingShells/Candler/compare/schemas@0.3.0...schemas@0.4.0) - 2026-11-09

### Features

- *(schemas)* record the import rules digest in the run manifest
- *(schemas)* frame evaluator messages as json-rpc 2.0
- *(evaluator-protocol)* mint request ids as uuid v7

### Fixes

- *(evaluation)* read a suite file written as json

## [0.3.0](https://github.com/CrackingShells/Candler/compare/schemas@0.2.0...schemas@0.3.0) - 2026-11-09

### Features

- *(schemas)* [**breaking**] add container records and the M2b event kinds

## [0.2.0](https://github.com/CrackingShells/Candler/compare/schemas@0.1.0...schemas@0.2.0) - 2026-11-05

### Documentation

- *(readme)* describe the m1 crates, fixtures and schema roots

### Features

- *(schemas)* construct non-exhaustive wire types from outside
- *(schemas)* publish the execution schemas
- *(schemas)* define the job record and the event envelope
- *(schemas)* define the workspace tree of a run
- *(schemas)* define the run manifest and its cell digest
- *(schemas)* define the experiment file and budget
- *(schemas)* publish the evaluator protocol schemas
- *(schemas)* define evaluate params, run context and notifications
- *(schemas)* define the describe result and evaluator needs
- *(schemas)* define the plug-in manifest and the suite file
- *(schemas)* add plain names to the shared primitives
- *(schemas)* add secret names to the shared primitives
- *(schemas)* publish the raw frame and trace schemas
- *(schemas)* define the trace file and its ordering rules
- *(schemas)* define normalised trace events
- *(schemas)* define sessions and how a session ends
- *(schemas)* define raw frames and stream names
- *(schemas)* publish the child record schema
- *(schemas)* define the child record and exit status
- *(schemas)* canonicalise JSON values per RFC 8785
- *(schemas)* add job ids, field paths and resolved values

### Refactoring

- *(schemas)* name trace sources with the shared plain name
