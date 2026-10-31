use latexide_contracts::{
    ContractError, RevisionedEdit, SnapshotFile, SnapshotLimits, SourceAnchor, Utf16Range,
    WorkspaceSnapshot, validate_edits,
};

fn snapshot() -> WorkspaceSnapshot {
    WorkspaceSnapshot { id: "snapshot-7".into(), epoch: 7, files: vec![SnapshotFile {
        path: "main.tex".into(), revision: Some(7), content_sha256: "a".repeat(64),
        utf8: "A😀\né".as_bytes().to_vec(), dirty: true,
    }] }
}

fn limits() -> SnapshotLimits { SnapshotLimits { max_files: 20, max_file_bytes: 1024 * 1024, max_total_bytes: 4 * 1024 * 1024 } }

#[test]
fn canonical_snapshot_roundtrips_large_revisions_and_utf8_without_platform_paths() {
    let mut value = snapshot(); value.files[0].revision = Some(9_007_199_254_740_993);
    assert!(value.validate(&limits()).is_ok());
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(serde_json::from_slice::<WorkspaceSnapshot>(&bytes).unwrap(), value);
}

#[test]
fn a_snapshot_can_include_more_files_than_the_ten_retained_editor_buffers() {
    let mut value = snapshot();
    for n in 0..11 { let mut file = value.files[0].clone(); file.path = format!("chapter-{n}.tex"); file.revision = None; file.dirty = false; value.files.push(file); }
    assert!(value.validate(&limits()).is_ok());
    let mut budget = limits(); budget.max_files = 10;
    assert_eq!(value.validate(&budget), Err(ContractError::Quota));
}

#[test]
fn snapshot_rejects_noncanonical_utf8_duplicates_and_unrevisioned_dirty_content() {
    for content in [vec![0xff], b"a\r\nb".to_vec(), b"a\0b".to_vec()] {
        let mut value = snapshot(); value.files[0].utf8 = content;
        assert_eq!(value.validate(&limits()), Err(ContractError::InvalidPayload));
    }
    let mut value = snapshot(); value.files.push(value.files[0].clone());
    assert_eq!(value.validate(&limits()), Err(ContractError::InvalidPayload));
    let mut value = snapshot(); value.files[0].revision = None;
    assert_eq!(value.validate(&limits()), Err(ContractError::InvalidPayload));
}

#[test]
fn quotas_use_bytes_and_do_not_allow_raising_the_existing_single_file_ceiling() {
    let mut value = snapshot(); let mut budget = limits(); budget.max_total_bytes = 2;
    assert_eq!(value.validate(&budget), Err(ContractError::Quota));
    budget = limits(); budget.max_file_bytes = 2 * 1024 * 1024;
    value.files[0].utf8 = vec![b'a'; 1024 * 1024 + 1];
    assert_eq!(value.validate(&budget), Err(ContractError::Quota));
}

#[test]
fn anchors_check_utf16_scalar_boundaries_line_column_revision_and_hash() {
    let value = snapshot();
    let anchor = SourceAnchor { document_id: "main.tex".into(), revision: Some(7), content_sha256: "a".repeat(64),
        line: 0, character: 1, range: Utf16Range { start: 1, length: 2 } };
    assert!(anchor.validate_in(&value).is_ok());
    let mut bad = anchor.clone(); bad.range.start = 2;
    assert_eq!(bad.validate_in(&value), Err(ContractError::InvalidRange));
    let mut bad = anchor.clone(); bad.line = 1;
    assert_eq!(bad.validate_in(&value), Err(ContractError::InvalidRange));
    let mut bad = anchor.clone(); bad.revision = Some(6);
    assert_eq!(bad.validate_in(&value), Err(ContractError::StaleRevision));
    let mut bad = anchor.clone(); bad.content_sha256 = "b".repeat(64);
    assert_eq!(bad.validate_in(&value), Err(ContractError::StaleSnapshot));
    let mut bad = anchor; bad.range = Utf16Range { start: u32::MAX, length: 2 };
    assert_eq!(bad.validate_in(&value), Err(ContractError::InvalidRange));
}

#[test]
fn edit_batches_validate_all_inputs_without_mutating_the_snapshot() {
    let value = snapshot(); let before = value.clone();
    let good = RevisionedEdit { document_id: "main.tex".into(), expected_revision: 7, start_utf16: 1, length_utf16: 2, replacement: "ő".into() };
    assert!(validate_edits(&value, &[good.clone()]).is_ok());
    let mut stale = good.clone(); stale.expected_revision = 6;
    assert_eq!(validate_edits(&value, &[good.clone(), stale]), Err(ContractError::StaleRevision));
    let mut overlapping = good.clone(); overlapping.start_utf16 = 1; overlapping.length_utf16 = 0;
    assert_eq!(validate_edits(&value, &[good.clone(), overlapping]), Err(ContractError::InvalidRange));
    let mut oversized = good; oversized.replacement = "a".repeat(1024 * 1024);
    assert_eq!(validate_edits(&value, &[oversized]), Err(ContractError::Quota));
    assert_eq!(value, before);
}

#[test]
fn largest_reverse_order_batch_preserves_unicode_ranges_and_error_precedence() {
    let mut value = snapshot();
    let blocks = 1024 * 1024 / 8;
    value.files[0].utf8 = "a😀é\n".repeat(blocks).into_bytes();
    let edits: Vec<_> = (0..1000).map(|index| RevisionedEdit {
        document_id: "main.tex".into(), expected_revision: 7,
        start_utf16: ((blocks - index - 1) * 5) as u32,
        length_utf16: 1, replacement: "b".into(),
    }).collect();
    let before = value.clone();
    let started = std::time::Instant::now();
    assert_eq!(validate_edits(&value, &edits), Ok(()));
    eprintln!("1 MiB / 1000 reverse edits validation: {:?}", started.elapsed());
    let mut split_scalar = edits[..2].to_vec();
    split_scalar[1].start_utf16 += 2;
    assert_eq!(validate_edits(&value, &split_scalar), Err(ContractError::InvalidRange));
    let mut first_invalid = edits[..2].to_vec();
    first_invalid[0].start_utf16 = u32::MAX;
    first_invalid[1].expected_revision = 6;
    assert_eq!(validate_edits(&value, &first_invalid), Err(ContractError::InvalidRange));
    assert_eq!(value, before);
}
