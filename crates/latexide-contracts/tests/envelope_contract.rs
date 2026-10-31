use latexide_contracts::{CallContext, ContractEnvelope, ContractError, ContractRequest, decode_request};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request { snapshot: String, text: String }
impl ContractRequest for Request {
    fn package_id(&self) -> &str { "P07" }
    fn snapshot_id(&self) -> Option<&str> { Some(&self.snapshot) }
}

fn bytes() -> Vec<u8> {
    serde_json::to_vec(&ContractEnvelope::new("request-1", Request { snapshot: "s1".into(), text: "Árvíz 😀".into() })).unwrap()
}
fn context() -> CallContext { CallContext { active_snapshot: Some("s1".into()), ..Default::default() } }

#[test]
fn known_v1_envelope_roundtrips_and_is_independent_from_legacy_ipc_v1() {
    let decoded = decode_request::<Request>(&bytes(), &context()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), serde_json::from_slice::<serde_json::Value>(&bytes()).unwrap());
    assert_eq!(decode_request::<Request>(br#"{"version":1,"token":"synthetic","request":{"op":"workspace_info"}}"#, &context()), Err(ContractError::MalformedJson));
}

#[test]
fn version_is_checked_before_unknown_future_payload_fields() {
    let input = br#"{"version":{"major":2,"minor":0},"package_id":"P07","request_id":"r","payload":{"future":true}}"#;
    assert_eq!(decode_request::<Request>(input, &context()), Err(ContractError::IncompatibleVersion));
    let extended = br#"{"version":{"major":2,"minor":0,"future_features":[]},"payload":{"future":true}}"#;
    assert_eq!(decode_request::<Request>(extended, &context()), Err(ContractError::IncompatibleVersion));
    let mut input: serde_json::Value = serde_json::from_slice(&bytes()).unwrap(); input["version"]["minor"] = 1.into();
    assert_eq!(decode_request::<Request>(&serde_json::to_vec(&input).unwrap(), &context()), Err(ContractError::IncompatibleVersion));
}

#[test]
fn caller_observed_stale_cancel_deadline_and_quota_states_are_distinct() {
    let mut ctx = context(); ctx.active_snapshot = Some("s2".into());
    assert_eq!(decode_request::<Request>(&bytes(), &ctx), Err(ContractError::StaleSnapshot));
    ctx = context(); ctx.cancelled = true;
    assert_eq!(decode_request::<Request>(&bytes(), &ctx), Err(ContractError::Cancelled));
    ctx = context(); ctx.deadline_ms = Some(10); ctx.elapsed_ms = 10;
    assert_eq!(decode_request::<Request>(&bytes(), &ctx), Err(ContractError::Deadline));
    ctx = context(); ctx.max_payload_bytes = 1;
    assert_eq!(decode_request::<Request>(&bytes(), &ctx), Err(ContractError::PayloadTooLarge));
}

#[test]
fn package_snapshot_and_request_id_bindings_cannot_silently_change() {
    let original: serde_json::Value = serde_json::from_slice(&bytes()).unwrap();
    for (field, value, expected) in [
        ("package_id", serde_json::json!("P08"), ContractError::WrongPackage),
        ("snapshot_id", serde_json::Value::Null, ContractError::StaleSnapshot),
        ("request_id", serde_json::json!(""), ContractError::InvalidPayload),
    ] {
        let mut input = original.clone(); input[field] = value;
        assert_eq!(decode_request::<Request>(&serde_json::to_vec(&input).unwrap(), &context()), Err(expected));
    }
    assert_eq!(decode_request::<Request>(b"{", &context()), Err(ContractError::MalformedJson));
}
