use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractVersion {
    pub major: u32,
    pub minor: u32,
}

impl ContractVersion {
    pub const V1: Self = Self { major: 1, minor: 0 };
    pub fn supported(self) -> bool { self == Self::V1 }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractError {
    UnknownRequirement,
    ForeignRequirement,
    ForeignPath,
    InvalidManifest,
    InvalidRegistry,
    IncompatibleVersion,
    CyclicRequirement,
    MalformedJson,
    PayloadTooLarge,
    WrongPackage,
    InvalidPayload,
    StaleSnapshot,
    StaleRevision,
    InvalidRange,
    Quota,
    Cancelled,
    Deadline,
    SourceMismatch,
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ContractError {}
