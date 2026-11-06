//! Jobs: the job directory and its locks, the slots, the lost-job recovery, the supervisor that
//! runs a job and the orchestration of its runs.
//!
//! A job is one invocation of an experiment. Its directory under the store holds the event log,
//! the records of the children and containers it started, and the slots that bound its parallelism.

pub mod dir;
pub mod events;
pub mod lock;
pub mod lost;
pub mod model;
pub mod orchestrate;
pub mod runtime;
pub mod slots;
pub mod view;

pub use self::runtime::supervise_entry;
