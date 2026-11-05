//! Candler core: resolution in [`resolve`], process supervision in [`supervise`], raw frame files
//! in [`frames`], trace normalisation in [`trace`], the run store in [`store`], and the legacy
//! importer in [`import`].

pub mod frames;
pub mod import;
pub mod resolve;
pub mod store;
#[cfg(unix)]
pub mod supervise;
pub mod trace;
