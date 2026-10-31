use latexide_contracts::{
    CallContext, ContractEnvelope, ContractError, SnapshotFile, SnapshotLimits, WorkspaceSnapshot,
    decode_request, editor::PlanEditorActionRequest, verticals::{QualifyVerticalRequest, VerticalManifest},
};

fn vertical(id: &str) -> QualifyVerticalRequest {
    QualifyVerticalRequest {
        manifest: VerticalManifest { id: id.into(), sdk_major: 1, tool_versions: vec![], required_capabilities: vec![] },
        results: vec![],
    }
}

#[test]
fn a_vertical_payload_cannot_impersonate_a_core_package() {
    for id in ["P00", "P18"] {
        let mut value = serde_json::to_value(ContractEnvelope::new("request", vertical("U05"))).unwrap();
        value["package_id"] = id.into();
        value["payload"]["manifest"]["id"] = id.into();
        assert_eq!(decode_request::<QualifyVerticalRequest>(&serde_json::to_vec(&value).unwrap(), &CallContext::default()), Err(ContractError::WrongPackage));
    }
    for id in ["U01", "U20"] {
        let bytes = serde_json::to_vec(&ContractEnvelope::new("request", vertical(id))).unwrap();
        assert!(decode_request::<QualifyVerticalRequest>(&bytes, &CallContext::default()).is_ok());
    }
}

#[test]
fn a_vertical_handler_checks_its_trusted_target_without_changing_the_wire_shape() {
    let encoded = serde_json::to_vec(&ContractEnvelope::new("request", vertical("U07"))).unwrap();
    assert_eq!(latexide_contracts::decode_request_for::<QualifyVerticalRequest>("U05", &encoded, &CallContext::default()), Err(ContractError::WrongPackage));
    let decoded = latexide_contracts::decode_request_for::<QualifyVerticalRequest>("U07", &encoded, &CallContext::default()).unwrap();
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), encoded);
}

#[test]
fn a_valid_in_memory_snapshot_does_not_bypass_the_encoded_v1_frame_budget() {
    let snapshot = WorkspaceSnapshot { id: "snapshot".into(), epoch: 1, files: vec![SnapshotFile {
        path: "main.tex".into(), revision: Some(1), content_sha256: "a".repeat(64),
        utf8: vec![b'a'; 1024 * 1024], dirty: false,
    }] };
    assert!(snapshot.validate(&SnapshotLimits { max_files: 1, max_file_bytes: 1024 * 1024, max_total_bytes: 1024 * 1024 }).is_ok());
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../tests/contracts/v1/p07.json")).unwrap();
    let mut request: ContractEnvelope<PlanEditorActionRequest> = serde_json::from_value(fixture["cases"][0]["request"].clone()).unwrap();
    request.snapshot_id = Some(snapshot.id.clone());
    request.payload.snapshot = snapshot;
    let encoded = serde_json::to_vec(&request).unwrap();
    assert!(encoded.len() > 1024 * 1024);
    // The caller may lower the budget, but cannot lift v1's frozen hard ceiling.
    let context = CallContext { active_snapshot: Some("snapshot".into()), max_payload_bytes: usize::MAX, ..Default::default() };
    assert_eq!(decode_request::<PlanEditorActionRequest>(&encoded, &context), Err(ContractError::PayloadTooLarge));
}
