# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
