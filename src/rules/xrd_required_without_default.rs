//! `xrd-required-without-default`: a `required` spec field without a schema default.

use super::{Doc, Rule};
use crate::diagnostic::{Diagnostic, Severity};
use crate::model::Resource;
use crate::yamlpath::YamlPath;
use serde_yaml_ng::Value;

pub struct XrdRequiredWithoutDefault;

pub const ID: &str = "xrd-required-without-default";

impl Rule for XrdRequiredWithoutDefault {
    fn id(&self) -> &'static str {
        ID
    }
    fn description(&self) -> &'static str {
        "XRD spec field listed in required has no schema default"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
        let Resource::Xrd(xrd) = &doc.document.resource else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (i, v) in xrd.spec.versions.iter().enumerate() {
            let Some(root) = v
                .schema
                .as_ref()
                .and_then(|s| s.open_api_v3_schema.as_ref())
            else {
                continue;
            };
            let Some(spec) = root.get("properties").and_then(|p| p.get("spec")) else {
                continue;
            };
            let base = YamlPath::new()
                .key("spec")
                .key("versions")
                .idx(i)
                .key("schema")
                .key("openAPIV3Schema")
                .key("properties")
                .key("spec");
            walk(doc, spec, &base, "spec", &mut out);
        }
        out
    }
}

/// Check `required` of `schema`, then recurse into object properties.
fn walk(doc: &Doc, schema: &Value, path: &YamlPath, field: &str, out: &mut Vec<Diagnostic>) {
    let props = schema.get("properties");
    if let Some(Value::Sequence(req)) = schema.get("required") {
        for (j, name) in req.iter().enumerate() {
            let Some(name) = name.as_str() else { continue };
            let has_default = props
                .and_then(|p| p.get(name))
                .is_some_and(|s| s.get("default").is_some());
            if !has_default {
                out.push(doc.diagnostic(
                    ID,
                    Severity::Warning,
                    &path.key("required").idx(j),
                    format!("field '{field}.{name}' is required but has no default"),
                ));
            }
        }
    }
    if let Some(Value::Mapping(m)) = props {
        for (k, child) in m {
            if let Some(k) = k.as_str() {
                walk(
                    doc,
                    child,
                    &path.key("properties").key(k),
                    &format!("{field}.{k}"),
                    out,
                );
            }
        }
    }
}
