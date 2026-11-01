//! Read-only checkpoint validation and acknowledged journal replay for contract v1.
//!
//! `RecoveryService::new` takes the last sequence acknowledged by a trusted source.
//! A complete checkpoint plus a contiguous journal suffix must reach it exactly.
//! Extra, missing or damaged records return `Corrupt`; no implicit tail salvage.
//! The caller can explicitly retry an independently acknowledged complete prefix.
//!
//! Journal checksum = lowercase SHA-256 of the exact *logical UTF-8 bytes* only.
//! It is an integrity check, not authentication of the sequence, revision or source.
//! A checkpoint has no checksum, serialization metadata, undo or disk baseline in
//! v1: the producer must bind/verify those before this port is used in a runtime.
//! Current disk hashes are raw-byte observations. They cannot establish a baseline
//! or turn recovered text into a save payload. This provider never writes files,
//! restores a runtime, clears dirty from a disk hash, or opens editor buffers.

mod sha256;

use latexide_contracts::{
    is_relative_path, is_sha256,
    recovery::{Checkpoint, DiskIdentity, JournalEntry, RecoveryError, RecoveryPort, RecoverySet},
};
use std::collections::{BTreeMap, BTreeSet};

// These limits bound this in-memory replay, not a new transport/editor quota.
const MAX_REPLAY_BYTES: usize = 16 * 1024 * 1024;
const MAX_JOURNAL_ENTRIES: usize = 1024;
const MAX_DISK_IDENTITIES: usize = 1024;
const MAX_PATH_BYTES: usize = 4096;

pub struct RecoveryService {
    acknowledged_sequence: u64,
}

impl RecoveryService {
    /// The watermark is caller-observed durable acknowledgement, never inferred
    /// from the final record present or from the maximum revision in a payload.
    pub fn new(acknowledged_sequence: u64) -> Self {
        Self {
            acknowledged_sequence,
        }
    }
}

impl RecoveryPort for RecoveryService {
    fn recover(
        &self,
        checkpoint: Checkpoint,
        journal: &[JournalEntry],
        disk: &[DiskIdentity],
    ) -> Result<RecoverySet, RecoveryError> {
        if checkpoint.schema != 1 {
            return Err(RecoveryError::UnsupportedSchema);
        }
        if checkpoint.documents.len() > latexide_core::MAX_OPEN_DOCUMENTS
            || journal.len() > MAX_JOURNAL_ENTRIES
            || disk.len() > MAX_DISK_IDENTITIES
        {
            return Err(RecoveryError::Quota);
        }
        let mut bytes = 0usize;
        for content in checkpoint
            .documents
            .iter()
            .map(|d| &d.utf8)
            .chain(journal.iter().map(|e| &e.utf8))
        {
            if content.len() > latexide_core::MAX_FILE_BYTES {
                return Err(RecoveryError::Quota);
            }
            bytes = bytes
                .checked_add(content.len())
                .ok_or(RecoveryError::Quota)?;
            if bytes > MAX_REPLAY_BYTES {
                return Err(RecoveryError::Quota);
            }
        }
        // No data has been applied or exposed if a later validation fails.
        if checkpoint.sequence > self.acknowledged_sequence {
            return Err(RecoveryError::Corrupt);
        }
        let mut paths = BTreeSet::new();
        for identity in disk {
            validate_path(&identity.path)?;
            if !is_sha256(&identity.content_sha256) || !paths.insert(&identity.path) {
                return Err(RecoveryError::Corrupt);
            }
        }
        let mut documents = checkpoint.documents;
        let mut indices = BTreeMap::new();
        for (index, document) in documents.iter().enumerate() {
            validate_path(&document.document_id)?;
            validate_text(&document.utf8)?;
            if document.revision == 0
                || indices
                    .insert(document.document_id.clone(), index)
                    .is_some()
            {
                return Err(RecoveryError::Corrupt);
            }
        }
        let mut last_sequence = checkpoint.sequence;
        for entry in journal {
            validate_path(&entry.document_id)?;
            validate_text(&entry.utf8)?;
            if last_sequence.checked_add(1) != Some(entry.sequence)
                || entry.sequence > self.acknowledged_sequence
                || !is_sha256(&entry.checksum)
                || sha256::hex(&entry.utf8) != entry.checksum
            {
                return Err(RecoveryError::Corrupt);
            }
            let index = indices
                .get(&entry.document_id)
                .ok_or(RecoveryError::Corrupt)?;
            let document = &mut documents[*index];
            if document.revision.checked_add(1) != Some(entry.revision) {
                return Err(RecoveryError::Corrupt);
            }
            document.revision = entry.revision;
            document.utf8.clone_from(&entry.utf8);
            // The journal is acknowledged unsaved work. It has no save marker.
            document.dirty = true;
            last_sequence = entry.sequence;
        }
        if last_sequence != self.acknowledged_sequence {
            return Err(RecoveryError::Corrupt);
        }
        Ok(RecoverySet {
            documents,
            last_sequence,
        })
    }
}

fn validate_path(path: &str) -> Result<(), RecoveryError> {
    if path.len() > MAX_PATH_BYTES {
        return Err(RecoveryError::Quota);
    }
    if !is_relative_path(path, false) {
        return Err(RecoveryError::Corrupt);
    }
    Ok(())
}

fn validate_text(bytes: &[u8]) -> Result<(), RecoveryError> {
    let text = std::str::from_utf8(bytes).map_err(|_| RecoveryError::Corrupt)?;
    // Physical BOM removal / CRLF normalization belongs to the producer. A
    // logical leading U+FEFF and combining scalars are content and stay intact.
    if text.contains(['\0', '\r']) {
        return Err(RecoveryError::Corrupt);
    }
    Ok(())
}

/// Trusted adapter labels. This is no OS lock or authenticity mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryScope {
    pub project_id: String,
    pub session_id: String,
}

/// A source must verify checkpoint integrity, framing and its acknowledgement
/// receipt before returning these typed values. A truncated frame is an error,
/// not a reason to omit an acknowledged record. v1 has no such byte reader.
#[derive(Debug, Clone)]
pub struct RecoveryInput {
    pub scope: RecoveryScope,
    pub checkpoint: Checkpoint,
    pub journal: Vec<JournalEntry>,
}

pub trait RecoverySource {
    fn read(&self) -> Result<RecoveryInput, std::io::ErrorKind>;
}

#[derive(Debug, PartialEq)]
pub enum SourceError {
    Unavailable(std::io::ErrorKind),
    WrongScope,
    Recovery(RecoveryError),
}

/// Read-only source adapter. The expected scope and acknowledgement come from
/// trusted caller state, not from fields used to authorize themselves.
/// Native ownership/locking, writer sync, durability and restoration are unrun.
pub fn recover_from_source(
    expected: &RecoveryScope,
    acknowledged_sequence: u64,
    source: &impl RecoverySource,
    disk: &[DiskIdentity],
) -> Result<RecoverySet, SourceError> {
    let input = source.read().map_err(SourceError::Unavailable)?;
    if input.scope != *expected {
        return Err(SourceError::WrongScope);
    }
    RecoveryService::new(acknowledged_sequence)
        .recover(input.checkpoint, &input.journal, disk)
        .map_err(SourceError::Recovery)
}
