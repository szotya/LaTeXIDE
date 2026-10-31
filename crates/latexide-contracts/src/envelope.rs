use crate::{ContractError, ContractVersion};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const MAX_CONTRACT_BYTES: usize = 1024 * 1024;

pub trait ContractRequest: Serialize + DeserializeOwned {
    fn package_id(&self) -> &str;
    fn snapshot_id(&self) -> Option<&str> { None }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractEnvelope<T> {
    pub version: ContractVersion,
    pub package_id: String,
    pub request_id: String,
    pub snapshot_id: Option<String>,
    pub payload: T,
}

impl<T: ContractRequest> ContractEnvelope<T> {
    pub fn new(request_id: impl Into<String>, payload: T) -> Self {
        Self { version: ContractVersion::V1, package_id: payload.package_id().into(),
            request_id: request_id.into(), snapshot_id: payload.snapshot_id().map(str::to_owned), payload }
    }
}

/// Observations supplied by the trusted caller, not decoded from a client.
/// This checks a checkpoint; it does not run a timer or cancel background work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallContext {
    pub active_snapshot: Option<String>,
    pub cancelled: bool,
    pub elapsed_ms: u64,
    pub deadline_ms: Option<u64>,
    pub max_payload_bytes: usize,
}
impl Default for CallContext {
    fn default() -> Self {
        Self { active_snapshot: None, cancelled: false, elapsed_ms: 0,
            deadline_ms: None, max_payload_bytes: MAX_CONTRACT_BYTES }
    }
}

pub fn decode_request<T: ContractRequest>(
    bytes: &[u8], context: &CallContext,
) -> Result<ContractEnvelope<T>, ContractError> {
    if bytes.len() > context.max_payload_bytes.min(MAX_CONTRACT_BYTES) {
        return Err(ContractError::PayloadTooLarge);
    }
    #[derive(Deserialize)]
    struct VersionProbe { major: u32, #[serde(default)] minor: u32 }
    #[derive(Deserialize)]
    struct Header { version: VersionProbe }
    let header: Header = serde_json::from_slice(bytes).map_err(|_| ContractError::MalformedJson)?;
    if header.version.major != 1 || header.version.minor != 0 { return Err(ContractError::IncompatibleVersion); }
    if context.cancelled { return Err(ContractError::Cancelled); }
    if context.deadline_ms.is_some_and(|limit| context.elapsed_ms >= limit) { return Err(ContractError::Deadline); }
    let request: ContractEnvelope<T> = serde_json::from_slice(bytes).map_err(|_| ContractError::MalformedJson)?;
    if !crate::integration::is_package_id(&request.package_id) || request.package_id != request.payload.package_id() {
        return Err(ContractError::WrongPackage);
    }
    if request.request_id.is_empty() || request.request_id.len() > 256 || request.request_id.chars().any(char::is_control) {
        return Err(ContractError::InvalidPayload);
    }
    let payload_snapshot = request.payload.snapshot_id();
    if request.snapshot_id.as_deref() != payload_snapshot
        || payload_snapshot.is_some_and(|id| context.active_snapshot.as_deref() != Some(id))
    { return Err(ContractError::StaleSnapshot); }
    Ok(request)
}
