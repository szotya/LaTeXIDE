// Test-only owned module. A real worker uses the same import style in its own
// services/<package>/mod.rs, which P00 later registers in the production library.
use latexide_contracts::{ContractError, SnapshotLimits, WorkspaceSnapshot};

pub fn inspect_snapshot(snapshot: &WorkspaceSnapshot, limits: &SnapshotLimits) -> Result<(usize, usize), ContractError> {
    snapshot.validate(limits)?;
    Ok((snapshot.files.len(), latexide_core::MAX_FILE_BYTES))
}
