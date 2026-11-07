use super::{Extra, Metadata};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

/// CompositeResourceDefinition; `apiVersion` is `apiextensions.crossplane.io/v1` or `/v2`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Xrd {
    pub api_version: String,
    pub kind: String,
    #[serde(default)]
    pub metadata: Metadata,
    pub spec: XrdSpec,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Xrd {
    /// True when `apiVersion` is the v2 API.
    pub fn is_v2(&self) -> bool {
        self.api_version.ends_with("/v2")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XrdSpec {
    pub group: String,
    pub names: XrdNames,
    /// v2 only: `Namespaced` or `Cluster`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// v1 only: claim names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_names: Option<XrdNames>,
    #[serde(default)]
    pub versions: Vec<XrdVersion>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XrdNames {
    pub kind: String,
    pub plural: String,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XrdVersion {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referenceable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<XrdVersionSchema>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdVersionSchema {
    /// Raw OpenAPI v3 schema; walked by later stages.
    #[serde(
        rename = "openAPIV3Schema",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub open_api_v3_schema: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}
