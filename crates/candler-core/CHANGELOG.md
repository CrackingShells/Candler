# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

### Fixes

- *(runner)* refuse a record whose group the process does not lead
- *(runner)* end what a stop leaves behind and bound what it waits for
- *(importer)* annotate every borrow in the legacy mapping

### Refactoring

- *(importer)* name the legacy format keys once
- *(importer)* take the resolved-version key from the rules
- *(run-store)* name the manifest mismatch text detail
