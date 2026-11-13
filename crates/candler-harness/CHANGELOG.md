# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
