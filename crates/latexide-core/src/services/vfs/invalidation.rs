//! Bounded host event hints. Events never carry disk bytes or publish snapshots.

use super::path;
use latexide_contracts::vfs::VfsError;

/// Independent of file quotas; every retained path is also at most 4096 bytes.
pub const MAX_PENDING_INVALIDATIONS: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub enum Invalidation<'a> {
    Path(&'a str),
    Rename { old: &'a str, new: &'a str },
    Overflow,
    Incomplete,
}

/// A session-local queue version, not an authenticated OS observation.
/// Callers may inspect the hints; acceptance requires the entire batch to match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidationBatch {
    generation: u64,
    pub paths: Vec<String>,
    pub full_reconcile: bool,
}

pub(super) struct InvalidationQueue {
    capacity: usize,
    generation: u64,
    paths: Vec<String>,
    full_reconcile: bool,
    exhausted: bool,
}

impl InvalidationQueue {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.min(MAX_PENDING_INVALIDATIONS),
            generation: 0,
            paths: Vec::new(),
            full_reconcile: false,
            exhausted: false,
        }
    }

    pub(super) fn batch(&self) -> Result<InvalidationBatch, VfsError> {
        if self.exhausted {
            return Err(VfsError::Quota);
        }
        Ok(InvalidationBatch {
            generation: self.generation,
            paths: self.paths.clone(),
            full_reconcile: self.full_reconcile,
        })
    }

    pub(super) fn matches(&self, batch: &InvalidationBatch) -> bool {
        !self.exhausted
            && batch.generation == self.generation
            && batch.paths == self.paths
            && batch.full_reconcile == self.full_reconcile
    }

    fn advance(&mut self) -> Result<(), VfsError> {
        match self.generation.checked_add(1) {
            Some(next) if !self.exhausted => {
                self.generation = next;
                Ok(())
            }
            _ => {
                self.exhausted = true;
                self.require_full();
                Err(VfsError::Quota)
            }
        }
    }

    pub(super) fn require_full(&mut self) {
        self.paths.clear();
        self.full_reconcile = true;
    }

    fn add_validated_path(&mut self, value: &str) {
        if self.full_reconcile || self.paths.iter().any(|item| item == value) {
            return;
        }
        if self.paths.len() == self.capacity {
            self.require_full();
        } else {
            self.paths.push(value.to_owned());
        }
    }

    pub(super) fn record(&mut self, event: Invalidation<'_>) -> Result<(), VfsError> {
        // A duplicate arriving after capture still makes that old batch stale.
        self.advance()?;
        let checked = match event {
            Invalidation::Path(value) => path(value),
            Invalidation::Rename { old, new } => path(old).and_then(|_| path(new)),
            Invalidation::Overflow | Invalidation::Incomplete => Ok(()),
        };
        if let Err(error) = checked {
            self.require_full();
            return Err(error);
        }
        match event {
            Invalidation::Path(value) => self.add_validated_path(value),
            Invalidation::Rename { old, new } => {
                // Validate both before inserting either. Capacity loss discards
                // all hints and requests a full scan, never a partial rename.
                self.add_validated_path(old);
                self.add_validated_path(new);
            }
            Invalidation::Overflow | Invalidation::Incomplete => self.require_full(),
        }
        Ok(())
    }

    pub(super) fn acknowledge_complete(&mut self) -> Result<(), VfsError> {
        self.advance()?;
        self.paths.clear();
        self.full_reconcile = false;
        Ok(())
    }
}
