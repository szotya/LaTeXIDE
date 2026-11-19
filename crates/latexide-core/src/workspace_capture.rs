//! Host-owned immutable capture of retained documents only.
//!
//! The identity is a project/process/instance/epoch correlation label. Neither
//! the label nor a SHA256 digest grants authentication, edit or Save authority.
//! Revalidate while holding the same Workspace owner/borrow as the consumer.
//! Unopened disk files and physical compiler-input capture are outside this API.
use super::Workspace;
use crate::{CoreError, MAX_FILE_BYTES, MAX_OPEN_DOCUMENTS, Result};
use crate::contracts::{ContractError, SnapshotFile, SnapshotLimits, WorkspaceSnapshot};
use crate::recovery::durable::raw_digest;
use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    path::Path,
    sync::{OnceLock, atomic::{AtomicU64, Ordering}},
};

static INSTANCE_GENERATION: AtomicU64 = AtomicU64::new(1);
static PROCESS_GENERATION: OnceLock<String> = OnceLock::new();
const MAX_CAPTURE_BYTES: usize = MAX_OPEN_DOCUMENTS * MAX_FILE_BYTES;

pub(super) struct CaptureState {
    project: String,
    process: &'static str,
    instance: u64,
    epoch: u64,
}
impl CaptureState {
    pub(super) fn new(root: &Path) -> Result<Self> {
        let instance = INSTANCE_GENERATION.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
            |value| value.checked_add(1)).map_err(|_| CoreError::new(
                "snapshot_generation_exhausted", "Workspace instance generation exhausted"))?;
        // Std's process-local randomized hash state separates process lifetimes;
        // the checked counter prevents instance reuse within this process.
        // This is deliberately not an authentication token or a CSPRNG API.
        let process = PROCESS_GENERATION.get_or_init(|| {
            let state = RandomState::new();
            format!("{:016x}{:016x}",
                state.hash_one(("latexide-workspace-process-v1",std::process::id(),0u8)),
                state.hash_one(("latexide-workspace-process-v1",std::process::id(),1u8)))
        });
        Ok(Self {
            project: raw_digest(root.as_os_str().as_encoded_bytes()),
            process,
            instance,
            epoch: 0,
        })
    }
    fn id(&self) -> String {
        format!("ws-{}-{}-{:016x}-{:016x}",self.project,self.process,self.instance,self.epoch)
    }
}

fn host_limits(limits: &SnapshotLimits) -> SnapshotLimits {
    SnapshotLimits {
        max_files: limits.max_files.min(MAX_OPEN_DOCUMENTS),
        max_file_bytes: limits.max_file_bytes.min(MAX_FILE_BYTES),
        max_total_bytes: limits.max_total_bytes.min(MAX_CAPTURE_BYTES),
    }
}
fn default_limits() -> SnapshotLimits {
    SnapshotLimits { max_files:MAX_OPEN_DOCUMENTS, max_file_bytes:MAX_FILE_BYTES,
        max_total_bytes:MAX_CAPTURE_BYTES }
}
fn quota() -> CoreError {
    CoreError::new("snapshot_quota", "Opened-document snapshot exceeds the host capture budget")
}

impl Workspace {
    // Internal host label only; analysis keeps an independent observation epoch.
    pub(super) fn analysis_host_identity(&self) -> String { self.capture.id() }

    /// Capture exact logical UTF8 from the actual retained documents.
    ///
    /// IDs, revisions, dirty flags and the generation/epoch are host observations;
    /// no caller supplies an identity or epoch. There is no disk read or Save.
    /// Physical BOM/CRLF remains in Document and is never serialized into utf8.
    pub fn transaction_snapshot(&self) -> Result<WorkspaceSnapshot> {
        self.transaction_snapshot_with_limits(&default_limits())
    }

    /// A consumer may lower capture budgets, never raise the host's existing
    /// per-file/open-document ceilings. Check sizes before cloning text bytes.
    pub fn transaction_snapshot_with_limits(&self, limits: &SnapshotLimits) -> Result<WorkspaceSnapshot> {
        let limits=host_limits(limits);
        if self.documents.len()>limits.max_files { return Err(quota()); }
        let mut total=0usize;
        for document in self.documents.values() {
            if document.text.len()>limits.max_file_bytes { return Err(quota()); }
            total=total.checked_add(document.text.len()).ok_or_else(quota)?;
            if total>limits.max_total_bytes { return Err(quota()); }
        }
        let files=self.documents.values().map(|document| SnapshotFile {
            path:document.id.clone(), revision:Some(document.revision),
            content_sha256:raw_digest(document.text.as_bytes()),
            utf8:document.text.as_bytes().to_vec(), dirty:document.text!=document.saved_text,
        }).collect();
        let snapshot=WorkspaceSnapshot { id:self.capture.id(),epoch:self.capture.epoch,files };
        snapshot.validate(&limits).map_err(|error| {
            if error==ContractError::Quota { quota() }
            else { CoreError::new("snapshot_invalid","Opened document cannot satisfy the v1 snapshot contract") }
        })?;
        Ok(snapshot)
    }

    /// Reject stale generations/epochs and compare every retained logical byte,
    /// path, revision, dirty flag and digest against a fresh owner capture.
    /// Equality is a data match, never authentication of an external caller.
    pub fn revalidate_transaction_snapshot(&self, expected: &WorkspaceSnapshot) -> Result<()> {
        if expected.epoch!=self.capture.epoch || expected.id!=self.capture.id()
            || self.transaction_snapshot()?!=*expected
        {
            return Err(CoreError::new("stale_snapshot","Workspace capture no longer matches this host"));
        }
        Ok(())
    }
    pub fn transaction_snapshot_matches(&self, expected: &WorkspaceSnapshot) -> bool {
        self.revalidate_transaction_snapshot(expected).is_ok()
    }

    // Revision binds all text mutations in the existing private Document API.
    // Dirty also observes Save, which changes no document revision. Keep only
    // small metadata here: never clone retained text or the undo history.
    fn capture_stamp(&self) -> Vec<(String,u64,bool)> {
        self.documents.values().map(|d|(d.id.clone(),d.revision,d.text!=d.saved_text)).collect()
    }
    pub(super) fn observe_capture_change<T>(&mut self, action: impl FnOnce(&mut Self)->Result<T>) -> Result<T> {
        let next=self.capture.epoch.checked_add(1).ok_or_else(|| CoreError::new(
            "snapshot_epoch_exhausted","Workspace capture epoch exhausted"))?;
        let before=self.capture_stamp();
        let result=action(self);
        if self.capture_stamp()!=before { self.capture.epoch=next; }
        // Preserve existing durable rollback/uncertain-commit and completed-save
        // error behavior. Any retained visible change must invalidate old data,
        // even if an existing recovery-finalization error is returned.
        result
    }
}
