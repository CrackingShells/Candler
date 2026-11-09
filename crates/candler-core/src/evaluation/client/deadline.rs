//! The clock the request deadline and the grace periods are measured on.
//!
//! The client reads time only through a [`Clock`], so that a test can drive the deadline without
//! waiting for it. [`SystemClock`] is the clock of production, on Tokio's timer.

use std::time::Duration;

use futures::future::BoxFuture;
use tokio::time::Instant;

/// A source of the current instant and of sleeps that end at an instant.
pub trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> Instant;

    /// A future that completes at `at`, or at once if `at` has passed.
    fn sleep_until(&self, at: Instant) -> BoxFuture<'_, ()>;
}

/// The clock of production: Tokio's timer.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, at: Instant) -> BoxFuture<'_, ()> {
        Box::pin(tokio::time::sleep_until(at))
    }
}

/// A future that completes `after` from now on `clock`.
pub(crate) fn sleep_for(clock: &dyn Clock, after: Duration) -> BoxFuture<'_, ()> {
    clock.sleep_until(clock.now() + after)
}
