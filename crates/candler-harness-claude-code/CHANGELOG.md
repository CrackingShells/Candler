# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.0](https://github.com/CrackingShells/Candler/compare/v0.17.0...v0.18.0) - 2026-11-25

### Features

- *(adapter-claude-code)* read a Bash call's exit status

### Fixes

- *(adapter-claude-code)* no exit for a nonzero result it does not flag
- *(adapter-claude-code)* clear the exit of a result that joins no call

### Refactoring

- *(adapter-claude-code)* pass no exit to the tool result

### Style

- *(adapter-claude-code)* wrap the tool result call as rustfmt does

## [0.17.0](https://github.com/CrackingShells/Candler/compare/v0.16.0...v0.17.0) - 2026-11-25

### Features

- *(api)* record loaded memory files on Claude Code
- *(api)* install instructions and hooks on Claude Code
- *(api)* drop the harness-id scaffolding and forward the check
- *(api)* make every driver state its harness check
- *(api)* read hooks and instructions as the harness writes them
- *(api)* declare instructions, hooks and the harness check

### Fixes

- *(api)* refuse only OpenCode check warnings, census path from workdir
- *(api)* fail the collection on a census error, fix a stream doc
- *(api)* accept a Claude Code config check only on a doctor report
- *(api)* read the hook log without following links

## [0.16.0](https://github.com/CrackingShells/Candler/compare/v0.15.0...v0.16.0) - 2026-11-25

### Documentation

- *(api)* generate pages from the declaration
- *(library)* say what the conformance collector is in the driver tests

### Features

- *(resolve)* [**breaking**] read MCP servers in one place and refuse a malformed one

### Fixes

- *(library)* refuse unknown effort and mode text on a stored cell
- *(library)* attribute MCP calls the same way on both harnesses
- *(library)* render MCP servers from their type in both drivers
- *(library)* ask nothing of a driver for a mode left unset
- *(library)* read every permission mode Claude Code reads
- *(library)* state each setting's path once and sweep the removed names

### Refactoring

- *(library)* read MCP servers through the library in both drivers
- *(library)* state each setting's path once, parse at resolve
- *(library)* read a cell's settings once and refuse unknown text
- *(library)* state every per-harness fact in one shape

## [0.12.3](https://github.com/CrackingShells/Candler/compare/v0.12.2...v0.12.3) - 2026-11-17

### Refactoring

- *(adapter-claude-code)* name the cell-path error at each reader
- *(library)* read a cell by text and as a table through Cell

## [0.12.0](https://github.com/CrackingShells/Candler/compare/v0.11.0...v0.12.0) - 2026-11-17

### Features

- *(adapter-claude-code)* give an MCP server its production name

## [0.10.0](https://github.com/CrackingShells/Candler/compare/v0.9.0...v0.10.0) - 2026-11-17

### Fixes

- *(runner)* render a plug-in's mcp servers for claude code

## [0.8.5](https://github.com/CrackingShells/Candler/compare/v0.8.4...v0.8.5) - 2026-11-13

### Refactoring

- *(library)* plan each run through one driver call

## [0.8.4](https://github.com/CrackingShells/Candler/compare/v0.8.3...v0.8.4) - 2026-11-13

### Refactoring

- *(isolation)* take the base image's harness version from a driver
- *(library)* read cells through one set of paths and accessors

## [0.8.3](https://github.com/CrackingShells/Candler/compare/v0.8.2...v0.8.3) - 2026-11-13

### Documentation

- *(contributing)* name tests by module path in the bless commands

### Refactoring

- *(library)* let the harness trait own what the CLI held

## [0.8.0](https://github.com/CrackingShells/Candler/compare/v0.7.0...v0.8.0) - 2026-11-13

### Features

- *(runner)* collect a harness's export and read it offline
- *(adapter-claude-code)* add session mode and the new driver facts

### Fixes

- *(adapter-claude-code)* cover Stop lines by order and prompt id
- *(adapter-claude-code)* tie each Stop line to its turn
- *(adapter-claude-code)* need a Stop line for every turn

### Refactoring

- *(runner)* take the run totals from the driver's live reader

## [0.7.0](https://github.com/CrackingShells/Candler/compare/v0.6.1...v0.7.0) - 2026-11-12

### Features

- *(cli)* code the refusals of the core and the harnesses

## [0.5.0](https://github.com/CrackingShells/Candler/compare/v0.4.1...v0.5.0) - 2026-11-10

### Refactoring

- *(repo)* [**breaking**] deny bare allow attributes and settle every suppression
- *(adapter-claude-code)* [**breaking**] build the driver's id and probe once

## [0.3.0](https://github.com/CrackingShells/Candler/compare/v0.2.0...v0.3.0) - 2026-11-09

### Features

- *(adapter-claude-code)* classify exits and collect outputs
- *(adapter-claude-code)* prepare settings, mcp config, skill tree
- *(adapter-claude-code)* build the reference command line
- *(orchestrator)* scaffold the M2b modules, traits and types

### Fixes

- *(adapter-claude-code)* archive only the treatment's skill subtree

### Style

- *(runner)* annotate the borrows, indexes and loops the review found
- *(adapter-claude-code)* name B21 in the guard, note the borrows

## [0.2.0](https://github.com/CrackingShells/Candler/compare/v0.1.0...v0.2.0) - 2026-11-05

### Documentation

- *(readme)* describe the m1 crates, fixtures and schema roots

### Features

- *(adapter-claude-code)* normalise sessions, subagents and denials
- *(adapter-claude-code)* detect and parse stream-json lines

### Refactoring

- *(adapter-claude-code)* build the reader with a fallible new

### Style

- *(adapter-claude-code)* word the borrow notes on as_str plainly
- *(adapter-claude-code)* annotate borrows, dyn types and the ratchet
