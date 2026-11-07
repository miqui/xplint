//! Typed models for Crossplane resources.
//!
//! Every typed struct keeps unknown fields in a flattened `extra` map so that
//! serializing a model reproduces the original document (round trip).

mod composition;
mod function;
mod xrd;

pub use composition::{CompositeTypeRef, Composition, CompositionSpec, FunctionRef, PipelineStep};
pub use function::{Function, FunctionSpec};
pub use xrd::{Xrd, XrdNames, XrdSpec, XrdVersion, XrdVersionSchema};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::BTreeMap;

/// Extra, untyped fields captured by `#[serde(flatten)]`.
pub type Extra = BTreeMap<String, Value>;

/// API group for XRDs and Compositions.
pub const APIEXTENSIONS_GROUP: &str = "apiextensions.crossplane.io";
/// API group for Functions.
pub const PKG_GROUP: &str = "pkg.crossplane.io";

/// Object metadata (subset used by lint rules).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A parsed resource: a known typed kind, or a generic document.
#[derive(Debug, Clone, PartialEq)]
pub enum Resource {
    Xrd(Xrd),
    Composition(Composition),
    Function(Function),
    /// Any other kind (or a non-mapping document), kept untouched.
    Generic(Value),
}

impl Resource {
    /// Classify a YAML value by `apiVersion`/`kind` and deserialize it.
    pub fn from_value(value: Value) -> Result<Resource, serde_yaml_ng::Error> {
        let api = value.get("apiVersion").and_then(Value::as_str);
        let kind = value.get("kind").and_then(Value::as_str);
        let (group, _version) = match api {
            Some(a) => split_api_version(a),
            None => ("", ""),
        };
        Ok(match (group, kind) {
            (APIEXTENSIONS_GROUP, Some("CompositeResourceDefinition")) => {
                Resource::Xrd(serde_yaml_ng::from_value(value)?)
            }
            (APIEXTENSIONS_GROUP, Some("Composition")) => {
                Resource::Composition(serde_yaml_ng::from_value(value)?)
            }
            (PKG_GROUP, Some("Function")) => Resource::Function(serde_yaml_ng::from_value(value)?),
            _ => Resource::Generic(value),
        })
    }

    /// Serialize back to a YAML value.
    pub fn to_value(&self) -> Result<Value, serde_yaml_ng::Error> {
        match self {
            Resource::Xrd(x) => serde_yaml_ng::to_value(x),
            Resource::Composition(c) => serde_yaml_ng::to_value(c),
            Resource::Function(f) => serde_yaml_ng::to_value(f),
            Resource::Generic(v) => Ok(v.clone()),
        }
    }

    /// Human-readable kind name.
    pub fn kind_name(&self) -> &str {
        match self {
            Resource::Xrd(_) => "CompositeResourceDefinition",
            Resource::Composition(_) => "Composition",
            Resource::Function(_) => "Function",
            Resource::Generic(v) => v.get("kind").and_then(Value::as_str).unwrap_or(""),
        }
    }
}

/// Split `group/version` (core resources have no group).
pub fn split_api_version(api_version: &str) -> (&str, &str) {
    match api_version.split_once('/') {
        Some((g, v)) => (g, v),
        None => ("", api_version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_api_version() {
        assert_eq!(
            split_api_version("apiextensions.crossplane.io/v2"),
            ("apiextensions.crossplane.io", "v2")
        );
        assert_eq!(split_api_version("v1"), ("", "v1"));
    }

    #[test]
    fn unknown_kind_is_generic() {
        let v: Value = serde_yaml_ng::from_str("apiVersion: v1\nkind: ConfigMap\n").unwrap();
        let r = Resource::from_value(v).unwrap();
        assert!(matches!(r, Resource::Generic(_)));
        assert_eq!(r.kind_name(), "ConfigMap");
    }

    #[test]
    fn scalar_document_is_generic() {
        let r = Resource::from_value(Value::String("hi".into())).unwrap();
        assert!(matches!(r, Resource::Generic(_)));
    }

    #[test]
    fn known_kind_missing_required_field_errors() {
        let v: Value = serde_yaml_ng::from_str(
            "apiVersion: pkg.crossplane.io/v1\nkind: Function\nmetadata: {name: f}\n",
        )
        .unwrap();
        assert!(Resource::from_value(v).is_err());
    }
}
