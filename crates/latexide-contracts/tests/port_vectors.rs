use latexide_contracts::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::path::Path;

fn check<T: ContractRequest, R: Serialize + DeserializeOwned, E: Serialize + DeserializeOwned>(id: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/contracts/v1").join(format!("{}.json", id.to_lowercase()));
    let data: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(data["package_id"], id);
    assert!(data["scope"].as_str().unwrap().contains("no service"));
    let cases = data["cases"].as_array().unwrap(); assert_eq!(cases.len(), 7);
    for case in cases {
        let context: CallContext = serde_json::from_value(case["context"].clone()).unwrap();
        let bytes = case.get("raw").and_then(Value::as_str).map(|s| s.as_bytes().to_vec())
            .unwrap_or_else(|| serde_json::to_vec(&case["request"]).unwrap());
        let actual = decode_request::<T>(&bytes, &context);
        match case["expect_boundary"].as_str().unwrap() {
            "ok" | "not_applicable" => {
                let request = actual.unwrap_or_else(|error| panic!("{id} {}: {error}", case["name"]));
                assert_eq!(serde_json::to_value(request).unwrap(), case["request"]);
                if case["expect_boundary"] == "not_applicable" { assert!(case["reason"].as_str().unwrap().contains("snapshot")); }
            }
            _ => {
                let expected: ContractError = serde_json::from_value(case["expect_boundary"].clone()).unwrap();
                assert!(matches!(actual, Err(ref error) if *error == expected), "{id} {}", case["name"]);
            }
        }
        if let Some(response) = case.get("response_example") {
            let typed: R = serde_json::from_value(response.clone()).unwrap();
            assert_eq!(serde_json::to_value(typed).unwrap(), *response);
        }
        if let Some(error) = case.get("service_error_example") {
            let typed: E = serde_json::from_value(error.clone()).unwrap();
            assert_eq!(serde_json::to_value(typed).unwrap(), *error);
        }
    }
}

#[test]
fn all_core_first_ports_have_typed_frozen_vectors() {
    check::<integration_port::ValidateManifestRequest, IntegrationOrder, ContractError>("P00");
    check::<recovery::RecoverRequest, recovery::RecoverySet, recovery::RecoveryError>("P01");
    check::<transactions::PrepareTransactionRequest, transactions::PreparedTransaction, transactions::TransactionError>("P02");
    check::<runtime::AdmitFrameRequest, runtime::AcceptedFrame, runtime::TransportError>("P03");
    check::<vfs::ReconcileRequest, vfs::VfsSnapshot, vfs::VfsError>("P04");
    check::<language::AnalyzeRequest, language::AnalysisResult, language::LanguageError>("P05");
    check::<navigation::SearchRequest, navigation::SearchPage, navigation::SearchError>("P06");
    check::<editor::PlanEditorActionRequest, editor::EditorTransaction, editor::EditorError>("P07");
    check::<workspace::ResolveSettingsRequest, workspace::EffectiveSettings, workspace::SettingsError>("P08");
    check::<build::EvaluateArtifactRequest, build::ArtifactState, build::ArtifactError>("P09");
    check::<pdf::ForwardSyncRequest, pdf::PdfLocation, pdf::PdfError>("P10");
    check::<research::PlanExportRequest, research::ExportManifest, research::ExportError>("P11");
    check::<bibliography::InspectBibliographyRequest, bibliography::BibliographyReport, ContractError>("P12");
    check::<git::ParseStatusV2Request, git::GitStatus, git::GitError>("P13");
    check::<assistant::ReduceAssistantRequest, assistant::AssistantState, assistant::AssistantError>("P14");
    check::<extensions::NegotiateProviderRequest, extensions::ProviderSession, extensions::ProviderError>("P15");
    check::<terminal::ValidateTerminalProfileRequest, terminal::TerminalLaunch, terminal::TerminalError>("P16");
    check::<developer_tools::TransitionTaskRequest, developer_tools::TaskState, developer_tools::TaskError>("P17");
    check::<security::AuthorizeResourceRequest, security::ResourceGrant, security::SecurityError>("P18");
    check::<quality_corpus::ExpectedFaultOutcomeRequest, quality_corpus::FaultExpectation, ContractError>("P19");
    check::<release::QualifyReleaseRequest, release::ReleaseDecision, release::ReleaseError>("P20");
    check::<windows::EvaluateWindowsEvidenceRequest, windows::WindowsQualification, PlatformError>("P21");
    check::<linux::ValidateRuntimeDirectoryRequest, String, PlatformError>("P22");
    check::<performance_accessibility::SummarizeSamplesRequest, performance_accessibility::TimingSummary, performance_accessibility::MeasurementError>("P23");
}

#[test]
fn each_optional_vertical_uses_the_same_typed_sdk_boundary_with_its_own_identity() {
    for n in 1..=20 {
        check::<verticals::QualifyVerticalRequest, verticals::VerticalQualification, verticals::VerticalError>(&format!("U{n:02}"));
    }
}
