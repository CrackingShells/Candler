# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.17.0](https://github.com/CrackingShells/Candler/compare/v0.16.0...v0.17.0) - 2026-11-25

### Features

- *(api)* record loaded memory files on Claude Code
- *(api)* install instructions and hooks on Claude Code
- *(api)* drop the harness-id scaffolding and forward the check
- *(api)* make every driver state its harness check
- *(api)* read hooks and instructions as the harness writes them
- *(api)* declare instructions, hooks and the harness check
- *(api)* take instructions and hooks as cell values

### Fixes

- *(api)* refuse only OpenCode check warnings, census path from workdir
- *(api)* read a harness check's findings whatever its exit
- *(api)* fail the collection on a census error, fix a stream doc
- *(runner)* tag the sidecar image by its sources, keep one stream

### Refactoring

- *(api)* share one gateway stream reader, ask the census as a bool

## [0.16.0](https://github.com/CrackingShells/Candler/compare/v0.15.0...v0.16.0) - 2026-11-25

### Documentation

- *(library)* point the name check at the published pattern
- *(library)* say where effort and mode text is checked and mapped

### Features

- *(resolve)* [**breaking**] read MCP servers in one place and refuse a malformed one

### Fixes

- *(library)* refuse unknown effort and mode text on a stored cell
- *(library)* accept only server names no harness rewrites or splits
- *(library)* attribute MCP calls the same way on both harnesses
- *(library)* render MCP servers from their type in both drivers
- *(library)* ask nothing of a driver for a mode left unset
- *(library)* ask the driver about the mode of a cell that names none
- *(library)* read every permission mode Claude Code reads
- *(library)* state each setting's path once and sweep the removed names
- *(experiment)* point an MCP server refusal at its field

### Refactoring

- *(library)* state each setting's path once, parse at resolve
- *(library)* read a cell's settings once and refuse unknown text
- *(library)* state every per-harness fact in one shape

## [0.14.0](https://github.com/CrackingShells/Candler/compare/v0.13.0...v0.14.0) - 2026-11-23

### Features

- *(resolve)* refuse a pinned commit that lacks the skill tree

## [0.12.4](https://github.com/CrackingShells/Candler/compare/v0.12.3...v0.12.4) - 2026-11-17

### Refactoring

- *(library)* check session ids and env names via primitives

## [0.12.3](https://github.com/CrackingShells/Candler/compare/v0.12.2...v0.12.3) - 2026-11-17

### Refactoring

- *(library)* name the cell-path error at each reader
- *(library)* read a cell by text and as a table through Cell

## [0.12.1](https://github.com/CrackingShells/Candler/compare/v0.12.0...v0.12.1) - 2026-11-17

### Refactoring

- *(runner)* check duplicates through the schemas helper

## [0.12.0](https://github.com/CrackingShells/Candler/compare/v0.11.0...v0.12.0) - 2026-11-17

### Features

- *(adapter-claude-code)* give an MCP server its production name

## [0.11.0](https://github.com/CrackingShells/Candler/compare/v0.10.1...v0.11.0) - 2026-11-17

### Features

- *(isolation)* build an extension from the treatment's tree

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

### Refactoring

- *(library)* let the harness trait own what the CLI held

## [0.8.0](https://github.com/CrackingShells/Candler/compare/v0.7.0...v0.8.0) - 2026-11-13

### Documentation

- *(runner)* say what a collect command sees

### Features

- *(runner)* declare the settings table a driver answers
- *(runner)* refuse a setting a harness cannot honour, naming its path
- *(runner)* code the census inventory refusals
- *(runner)* code the session-record and session-id refusals
- *(runner)* let a driver supply gateway, state, census and session

### Fixes

- *(runner)* refuse an unsupported setting when the cells are planned
- *(adapter-opencode)* refuse an unknown effort level as unsupported

### Refactoring

- *(runner)* consolidate opencode_wiring
- *(runner)* take the run totals from the driver's live reader
- *(runner)* [**breaking**] frame streams with the reader's own framing type

## [0.7.0](https://github.com/CrackingShells/Candler/compare/v0.6.1...v0.7.0) - 2026-11-12

### Features

- *(cli)* code the refusals of the core and the harnesses

## [0.5.0](https://github.com/CrackingShells/Candler/compare/v0.4.1...v0.5.0) - 2026-11-10

### Refactoring

- *(repo)* [**breaking**] deny bare allow attributes and settle every suppression
- *(adapter-claude-code)* [**breaking**] build the driver's id and probe once

## [0.2.0](https://github.com/CrackingShells/Candler/compare/v0.1.0...v0.2.0) - 2026-11-05

### Documentation

- *(readme)* describe the m1 crates, fixtures and schema roots

### Features

- *(runner)* define the harness driver and trace reader traits
- *(runner)* define commands, stop specs and collect specs
