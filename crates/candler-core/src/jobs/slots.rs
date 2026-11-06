//! The slots that bound how many runs of all jobs execute at once.

use std::path::Path;

use futures::future::BoxFuture;

/// Why a slot could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SlotError {
    /// The slots are not implemented.
    #[error("slots are not available: {detail}")]
    Unavailable {
        /// Which call.
        detail: String,
    },
}

/// Who holds a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Holder {
    /// The job that holds it.
    pub job: String,
}

/// A held slot; dropping it releases the slot.
#[derive(Debug)]
#[non_exhaustive]
pub struct Slot {
    /// The slot's number.
    pub index: u32,
}

/// What a slot looks like from outside.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SlotInfo {
    /// The slot's number.
    pub index: u32,
    /// Who holds it, if anyone.
    pub holder: Option<Holder>,
}

/// The slots of a store.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Slots {
    count: u32,
}

impl Slots {
    /// Opens the slots under `dir`, `count` of them.
    ///
    /// # Errors
    /// [`SlotError`] when the slot files cannot be opened.
    pub fn open(dir: &Path, count: u32) -> Result<Self, SlotError> {
        let _ = (dir, count);
        Err(SlotError::Unavailable {
            detail: "Slots::open".to_owned(),
        })
    }

    /// How many slots there are.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Waits for a free slot and takes it.
    ///
    /// # Errors
    /// [`SlotError`] when the slots cannot be read.
    pub fn acquire<'a>(&'a self, holder: &'a Holder) -> BoxFuture<'a, Result<Slot, SlotError>> {
        let _ = holder;
        Box::pin(async {
            Err(SlotError::Unavailable {
                detail: "Slots::acquire".to_owned(),
            })
        })
    }

    /// Takes a free slot, or returns `None` when there is none.
    ///
    /// # Errors
    /// [`SlotError`] when the slots cannot be read.
    pub fn try_acquire(&self, holder: &Holder) -> Result<Option<Slot>, SlotError> {
        let _ = holder;
        Err(SlotError::Unavailable {
            detail: "Slots::try_acquire".to_owned(),
        })
    }
}
