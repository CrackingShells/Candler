# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
