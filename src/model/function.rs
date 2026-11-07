use super::{Extra, Metadata};
use serde::{Deserialize, Serialize};

/// Crossplane package `Function` (`pkg.crossplane.io`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Function {
    pub api_version: String,
    pub kind: String,
    #[serde(default)]
    pub metadata: Metadata,
    pub spec: FunctionSpec,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSpec {
    /// OCI image reference of the function package.
    pub package: String,
    #[serde(flatten)]
    pub extra: Extra,
}
