//! `composition-missing-deletion-policy`: composed resources without an explicit `deletionPolicy`.
//!
//! Checks `base` of each resource in function-patch-and-transform style input
//! (`input.resources[*].base`). `managementPolicies` is the newer provider
//! mechanism that supersedes `deletionPolicy` (providers with management
//! policies enabled ignore/derive it), so a base whose `spec` sets
//! `managementPolicies` explicitly is treated as satisfying the rule. The
//! behavior differs across Crossplane v1/v2 providers; this is a deliberate,
//! conservative choice to avoid false positives.

use super::{Doc, Rule};
use crate::diagnostic::{Diagnostic, Severity};
use crate::yamlpath::YamlPath;

pub struct CompositionMissingDeletionPolicy;

pub const ID: &str = "composition-missing-deletion-policy";

impl Rule for CompositionMissingDeletionPolicy {
    fn id(&self) -> &'static str {
        ID
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
        let Some(comp) = doc.composition() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (i, step) in comp.spec.pipeline.iter().enumerate() {
            let Some(resources) = step
                .input
                .as_ref()
                .and_then(|inp| inp.get("resources"))
                .and_then(|r| r.as_sequence())
            else {
                continue;
            };
            for (j, res) in resources.iter().enumerate() {
                let Some(base) = res.get("base") else {
                    continue;
                };
                let spec = base.get("spec");
                let explicit = spec.is_some_and(|s| {
                    s.get("deletionPolicy").is_some() || s.get("managementPolicies").is_some()
                });
                if explicit {
                    continue;
                }
                let base_path = YamlPath::new()
                    .key("spec")
                    .key("pipeline")
                    .idx(i)
                    .key("input")
                    .key("resources")
                    .idx(j)
                    .key("base");
                let path = if spec.is_some() {
                    base_path.key("spec")
                } else {
                    base_path
                };
                let name = res.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                out.push(doc.diagnostic(
                    ID,
                    Severity::Warning,
                    &path,
                    format!(
                        "composed resource '{name}' sets neither deletionPolicy nor managementPolicies"
                    ),
                ));
            }
        }
        out
    }
}
