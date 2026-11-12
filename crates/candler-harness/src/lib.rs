//! Candler harness interface.
//!
//! A harness driver describes how to prepare, launch, observe, stop and collect one agent
//! harness, and a trace reader turns the harness's raw output into normalised sessions. Both are
//! synchronous and return data: a driver never spawns, signals or opens a socket, and the core
//! runs every command and HTTP call a driver describes.
//!
//! - [`spec`] holds the data a driver returns to the core.
//! - [`driver`] holds [`driver::HarnessDriver`] and its error.
//! - [`reader`] holds [`reader::TraceReader`], its frame input and its error.

#![deny(clippy::allow_attributes)]

pub mod driver;
pub mod reader;
pub mod refusal;
pub mod spec;
