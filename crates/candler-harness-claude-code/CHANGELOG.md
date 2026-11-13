# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
