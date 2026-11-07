use super::{Extra, Metadata};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

/// Composition (pipeline mode is the modeled path; other fields land in `extra`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Composition {
    pub api_version: String,
    pub kind: String,
    #[serde(default)]
    pub metadata: Metadata,
    pub spec: CompositionSpec,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionSpec {
    pub composite_type_ref: CompositeTypeRef,
    /// `Pipeline` for pipeline mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeline: Vec<PipelineStep>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl CompositionSpec {
    pub fn is_pipeline(&self) -> bool {
        self.mode.as_deref() == Some("Pipeline")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeTypeRef {
    pub api_version: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStep {
    pub step: String,
    pub function_ref: FunctionRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionRef {
    pub name: String,
}
