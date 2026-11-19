//! Bounded analysis over the frozen P04 real filesystem producer and this host.
//! No second document store: only an identity/digest/epoch is retained here.
//! Disk rows have revisionNone; matching clean open rows revisionSome/dirtyfalse;
//! dirty host shadows revisionSome/dirtytrue. Physical data stays in P04 receipts.
//! This synchronous producer is sequential local observation, not race isolation.
use super::{Workspace, analysis_vfs::{
    ObservationProducer, OverlayObservation, DiskBaseline,
    filesystem::{BoundedFilesystemProducer, HostOverlays, TraversalLimits},
}};
use crate::{CoreError, Result, MAX_FILE_BYTES, contracts::{
    SnapshotFile, SnapshotLimits, WorkspaceSnapshot, vfs::VfsError,
}};
use crate::recovery::durable::raw_digest;
use std::{collections::BTreeMap, fs, path::Path};

const MAX_FILES: usize = 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
type RootAnchor = Option<(u64,u64,u32)>;
fn root_anchor(root: &Path) -> Result<RootAnchor> {
    #[cfg(target_os="macos")] {
        use std::os::unix::fs::MetadataExt;
        let m=fs::symlink_metadata(root)?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(CoreError::new("analysis_outside_root","Analysis project root is no longer a directory"));
        }
        Ok(Some((m.dev(),m.ino(),m.uid())))
    }
    #[cfg(not(target_os="macos"))] { let _=root; Ok(None) }
}
pub(super) struct AnalysisState {
    root: RootAnchor,
    epoch: u64,
    binding: Option<String>,
}
impl AnalysisState {
    pub(super) fn new(root: &Path) -> Result<Self> {
        Ok(Self {root:root_anchor(root)?,epoch:0,binding:None})
    }
}
fn eligible(path: &str) -> bool {
    Path::new(path).extension().and_then(|x|x.to_str()).is_some_and(|ext|
        ["tex","ltx","sty","cls","bib"].iter().any(|e|ext.eq_ignore_ascii_case(e)))
}
fn limits(limits: &SnapshotLimits) -> SnapshotLimits {
    SnapshotLimits {max_files:limits.max_files.min(MAX_FILES),
        max_file_bytes:limits.max_file_bytes.min(MAX_FILE_BYTES),
        max_total_bytes:limits.max_total_bytes.min(MAX_BYTES)}
}
fn defaults() -> SnapshotLimits {
    SnapshotLimits {max_files:MAX_FILES,max_file_bytes:MAX_FILE_BYTES,max_total_bytes:MAX_BYTES}
}
fn error(e: VfsError) -> CoreError {
    let code=match e {VfsError::Quota=>"analysis_quota",VfsError::OutsideRoot=>"analysis_outside_root",
        _=>"analysis_incomplete"};
    CoreError::new(code,format!("Complete P04 analysis observation refused: {e:?}"))
}
struct DirtyHost<'a>(&'a Workspace);
impl HostOverlays for DirtyHost<'_> {
    fn snapshot(&self, limits: &SnapshotLimits) -> std::result::Result<Vec<OverlayObservation>,VfsError> {
        let mut size=0usize;let mut count=0usize;
        for d in self.0.documents.values().filter(|d|eligible(&d.id) && d.text!=d.saved_text) {
            count+=1;size=size.checked_add(d.text.len()).ok_or(VfsError::Quota)?;
            if count>limits.max_files || d.text.len()>limits.max_file_bytes || size>limits.max_total_bytes {
                return Err(VfsError::Quota);
            }
        }
        Ok(self.0.documents.values().filter(|d|eligible(&d.id) && d.text!=d.saved_text).map(|d|OverlayObservation {
            path:d.id.clone(),revision:d.revision,utf8:d.text.as_bytes().to_vec(),
            base:d.disk_identity.map(|(dev,ino)|DiskBaseline {
                identity:format!("unix:{dev}:{ino}"),content_sha256:raw_digest(d.saved_text.as_bytes()),
            }),
        }).collect())
    }
}
fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());out.extend_from_slice(bytes);
}
impl Workspace {
    /// Exact selected logical input set; editor buffer slots are never allocated.
    /// Bounded, sequential per-file analysis input, not a point-in-time disk view.
    /// `complete` means enumeration and each individual read completed; external
    /// writes between files may yield a mixed-time set. Its ID/revalidation bind
    /// the observed bytes only. Read-only analysis must tolerate that contract;
    /// edits use retained-document `transaction_snapshot` under the owner borrow.
    /// This API is not compiler-input, edit, Save or cross-file consistency authority.
    /// The P04 producer reads all regular files before projection:
    /// unsupported binary/nontext assets therefore refuse the WHOLE capture.
    pub fn analysis_snapshot(&mut self) -> Result<WorkspaceSnapshot> {
        self.analysis_snapshot_with_limits(&defaults())
    }
    pub fn analysis_snapshot_with_limits(&mut self, requested: &SnapshotLimits) -> Result<WorkspaceSnapshot> {
        let limits=limits(requested);
        if self.analysis.root!=root_anchor(&self.root)? {
            return Err(CoreError::new("analysis_outside_root","Workspace project identity changed"));
        }
        let (observation,receipt)={
            let producer=BoundedFilesystemProducer::new(self.root.clone(),DirtyHost(self),TraversalLimits::default()).map_err(error)?;
            let observation=producer.observe(&limits).map_err(error)?;
            let receipt=producer.last_capture().map_err(error)?;
            (observation,receipt)
        };
        if !observation.complete || !receipt.complete || receipt.failure.is_some() {
            return Err(CoreError::new("analysis_incomplete","Partial observation is not a snapshot"));
        }
        if self.analysis.root!=root_anchor(&self.root)? {
            return Err(CoreError::new("analysis_outside_root","Workspace project identity changed during observation"));
        }
        let raw: BTreeMap<_,_>=receipt.raw_files.iter().map(|f|(f.path.as_str(),f)).collect();
        let mut files=BTreeMap::new();
        for disk in observation.disk.into_iter().filter(|d|eligible(&d.path)) {
            let physical=raw.get(disk.path.as_str()).ok_or_else(||CoreError::new("analysis_incomplete","Missing raw provenance"))?;
            let revision=self.documents.get(&disk.path).filter(|d|
                d.text==d.saved_text && d.text.as_bytes()==disk.utf8 &&
                d.bom==physical.physical_bom &&
                (d.line_ending==crate::LineEnding::Crlf)==physical.crlf).map(|d|d.revision);
            files.insert(disk.path.clone(),SnapshotFile {path:disk.path,revision,
                content_sha256:raw_digest(&disk.utf8),utf8:disk.utf8,dirty:false});
        }
        for overlay in observation.overlays {
            files.insert(overlay.path.clone(),SnapshotFile {path:overlay.path,revision:Some(overlay.revision),
                content_sha256:raw_digest(&overlay.utf8),utf8:overlay.utf8,dirty:true});
        }
        let host=self.analysis_host_identity();
        let mut snapshot=WorkspaceSnapshot {id:format!("analysis-{host}"),epoch:self.analysis.epoch,files:files.into_values().collect()};
        snapshot.validate(&limits).map_err(|e|CoreError::new(
            if e==crate::contracts::ContractError::Quota {"analysis_quota"} else {"analysis_incomplete"},
            "Merged analysis snapshot violates its v1 limits"))?;
        let mut material=b"latexide-workspace-analysis-v1\0".to_vec();field(&mut material,host.as_bytes());
        for f in &snapshot.files {
            field(&mut material,f.path.as_bytes());material.push(u8::from(f.revision.is_some()));
            material.extend_from_slice(&f.revision.unwrap_or(0).to_be_bytes());material.push(u8::from(f.dirty));
            field(&mut material,&f.utf8);field(&mut material,f.content_sha256.as_bytes());
        }
        for f in receipt.raw_files.iter().filter(|f|eligible(&f.path)) {
            field(&mut material,f.path.as_bytes());field(&mut material,f.identity.as_bytes());
            material.extend_from_slice(&f.raw_size.to_be_bytes());field(&mut material,f.raw_sha256.as_bytes());
            material.push(u8::from(f.physical_bom));material.push(u8::from(f.crlf));
        }
        let binding=raw_digest(&material);
        if self.analysis.binding.as_ref()!=Some(&binding) {
            let next=self.analysis.epoch.checked_add(1).ok_or_else(||CoreError::new("analysis_quota","Analysis epoch exhausted"))?;
            self.analysis.binding=Some(binding);self.analysis.epoch=next;
        }
        snapshot.epoch=self.analysis.epoch;snapshot.id=format!("analysis-{host}-{:016x}",snapshot.epoch);
        Ok(snapshot)
    }
    pub fn revalidate_analysis_snapshot(&mut self, expected: &WorkspaceSnapshot) -> Result<()> {
        // A matching second sequential observation is a freshness check, not
        // proof that every disk file existed in this state at one instant.
        if self.analysis_snapshot()?!=*expected {
            return Err(CoreError::new("stale_analysis_snapshot","Complete analysis input no longer matches this Workspace"));
        }
        Ok(())
    }
    pub fn analysis_snapshot_matches(&mut self, expected: &WorkspaceSnapshot) -> bool {
        self.revalidate_analysis_snapshot(expected).is_ok()
    }
}
