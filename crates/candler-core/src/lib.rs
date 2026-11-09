//! Candler core: resolution in [`resolve`], process supervision in [`supervise`], raw frame files
//! in [`frames`], trace normalisation in [`trace`], the run store in [`store`], the legacy
//! importer in [`import`], container isolation in `isolation`, jobs in `jobs`, running one cell in
//! `runner`, and what an experiment file says in [`experiment`] and [`manifest`].

pub mod experiment;
pub mod frames;
pub mod import;
#[cfg(unix)]
pub mod isolation;
#[cfg(unix)]
pub mod jobs;
pub mod manifest;
pub mod resolve;
#[cfg(unix)]
pub mod runner;
pub mod store;
#[cfg(unix)]
pub mod supervise;
pub mod trace;
