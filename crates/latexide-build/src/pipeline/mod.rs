//! Read-only artifact decisions behind the frozen contract-v1 port.
//!
//! The host supplies a trusted completed-artifact record, current compiler identity,
//! and its authoritative immutable snapshot. Never construct the record by copying
//! an untrusted request: v1 has no fields proving completion or compiler provenance.
//! This module neither captures host inputs nor launches/controls compiler processes.
use latexide_contracts::{
    SnapshotLimits, WorkspaceSnapshot,
    build::{ArtifactError, ArtifactState, BuildManifest, BuildPort},
    common::is_sha256,
    integration::is_relative_path,
};
use std::collections::BTreeMap;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::CoreGraphicsChecks;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckError {
    InvalidData,
    Unavailable,
}

/// Real parsers/digest providers implement this boundary; byte-presence is insufficient.
pub trait ArtifactChecks {
    fn sha256(&self, bytes: &[u8]) -> Result<String, CheckError>;
    fn validate_pdf(&self, pdf: &[u8]) -> Result<(), CheckError>;
}

/// Explicit fail-closed option while a platform has no qualified parser adapter.
pub struct UnavailableChecks;
impl ArtifactChecks for UnavailableChecks {
    fn sha256(&self, _: &[u8]) -> Result<String, CheckError> {
        Err(CheckError::Unavailable)
    }
    fn validate_pdf(&self, _: &[u8]) -> Result<(), CheckError> {
        Err(CheckError::Unavailable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalPass {
    Complete,
    NeedsRerun,
    MissingDependency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildCompletion {
    Exited { code: i32, final_pass: FinalPass },
    Cancelled,
    TimedOut,
}

/// Host-owned provenance, outside v1 JSON. Bytes and every supplied input are bound
/// to this generation. A structurally valid DTO is not an authentic completion record.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactRecord {
    pub manifest: BuildManifest,
    pub snapshot_epoch: u64,
    pub pdf_sha256: String,
    pub completion: BuildCompletion,
}

#[derive(Debug, Clone)]
pub struct ArtifactLimits {
    pub snapshot: SnapshotLimits,
    pub max_pdf_bytes: usize,
}
impl Default for ArtifactLimits {
    fn default() -> Self {
        Self {
            snapshot: SnapshotLimits {
                max_files: 128,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 8 * 1024 * 1024,
            },
            max_pdf_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Borrows evidence only. There is no document store, save, artifact replacement or cleanup.
pub struct ArtifactEvaluator<'a, C: ArtifactChecks> {
    record: &'a ArtifactRecord,
    current_compiler_sha256: &'a str,
    checks: &'a C,
    limits: ArtifactLimits,
}
impl<'a, C: ArtifactChecks> ArtifactEvaluator<'a, C> {
    pub fn new(
        record: &'a ArtifactRecord,
        current_compiler_sha256: &'a str,
        checks: &'a C,
        limits: ArtifactLimits,
    ) -> Self {
        Self {
            record,
            current_compiler_sha256,
            checks,
            limits,
        }
    }
}

impl<C: ArtifactChecks> BuildPort for ArtifactEvaluator<'_, C> {
    fn evaluate_artifact(
        &self,
        manifest: &BuildManifest,
        current: &WorkspaceSnapshot,
        pdf: &[u8],
    ) -> Result<ArtifactState, ArtifactError> {
        use ArtifactError::{InconsistentSnapshot, InvalidPdf, MissingInput};
        let expected = validated_inputs(&self.record.manifest, self.limits.snapshot.max_files)?;
        let inputs = validated_inputs(manifest, self.limits.snapshot.max_files)?;
        if manifest.generation != self.record.manifest.generation
            || manifest.snapshot_id != self.record.manifest.snapshot_id
            || manifest.compiler_sha256 != self.record.manifest.compiler_sha256
            || inputs != expected
            || !is_sha256(self.current_compiler_sha256)
            || !is_sha256(&self.record.pdf_sha256)
            || !matches!(
                self.record.completion,
                BuildCompletion::Exited {
                    code: 0,
                    final_pass: FinalPass::Complete
                }
            )
        {
            return Err(InconsistentSnapshot);
        }
        current
            .validate(&self.limits.snapshot)
            .map_err(|_| InconsistentSnapshot)?;
        if pdf.is_empty() || pdf.len() > self.limits.max_pdf_bytes {
            return Err(InvalidPdf);
        }
        // A failed parse has precedence over byte-binding: exit0 + invalid bytes is InvalidPdf.
        self.checks.validate_pdf(pdf).map_err(|error| match error {
            CheckError::InvalidData => InvalidPdf,
            CheckError::Unavailable => InconsistentSnapshot,
        })?;
        if self.checks.sha256(pdf).map_err(|_| InconsistentSnapshot)? != self.record.pdf_sha256 {
            return Err(InconsistentSnapshot);
        }
        let mut files = BTreeMap::new();
        for file in &current.files {
            if self
                .checks
                .sha256(&file.utf8)
                .map_err(|_| InconsistentSnapshot)?
                != file.content_sha256
            {
                return Err(InconsistentSnapshot);
            }
            files.insert(file.path.as_str(), file.content_sha256.as_str());
        }
        let mut changed = false;
        for (path, digest) in &inputs {
            let current_digest = files.get(path).ok_or(MissingInput)?;
            changed |= current_digest != digest;
        }
        let same_snapshot = current.id == manifest.snapshot_id;
        if same_snapshot && (changed || current.epoch != self.record.snapshot_epoch) {
            return Err(InconsistentSnapshot);
        }
        Ok(
            if changed || !same_snapshot || manifest.compiler_sha256 != self.current_compiler_sha256
            {
                ArtifactState::Stale
            } else {
                ArtifactState::Current
            },
        )
    }
}

fn valid_identity(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn validated_inputs(
    manifest: &BuildManifest,
    max_files: usize,
) -> Result<BTreeMap<&str, &str>, ArtifactError> {
    let invalid = ArtifactError::InconsistentSnapshot;
    if !valid_identity(&manifest.generation)
        || !valid_identity(&manifest.snapshot_id)
        || !is_sha256(&manifest.compiler_sha256)
        || manifest.inputs.is_empty()
        || manifest.inputs.len() > max_files
    {
        return Err(invalid);
    }
    let mut inputs = BTreeMap::new();
    for input in &manifest.inputs {
        if !is_relative_path(&input.path, false)
            || !is_sha256(&input.content_sha256)
            || inputs
                .insert(input.path.as_str(), input.content_sha256.as_str())
                .is_some()
        {
            return Err(invalid);
        }
    }
    Ok(inputs)
}
