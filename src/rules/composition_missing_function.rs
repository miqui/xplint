//! `composition-missing-function`: pipeline step references a function that is not loaded.
//!
//! A function is "declared" when a `Function` resource with that `metadata.name`
//! is among the loaded documents. When no `Function` is loaded at all the rule
//! stays silent (like `patch-path-not-in-xrd` without an XRD), because linting a
//! Composition alone says nothing about where its functions live. A step with an
//! empty `functionRef.name` is always an error.

use super::{Doc, Rule};
use crate::diagnostic::{Diagnostic, Severity};
use crate::yamlpath::YamlPath;

pub struct CompositionMissingFunction;

pub const ID: &str = "composition-missing-function";

impl Rule for CompositionMissingFunction {
    fn id(&self) -> &'static str {
        ID
    }
    fn description(&self) -> &'static str {
        "Pipeline step references a function that no loaded Function declares"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
        let Some(comp) = doc.composition() else {
            return Vec::new();
        };
        let functions = doc.function_names();
        let mut out = Vec::new();
        for (i, step) in comp.spec.pipeline.iter().enumerate() {
            let name = step.function_ref.name.as_str();
            let path = YamlPath::new()
                .key("spec")
                .key("pipeline")
                .idx(i)
                .key("functionRef")
                .key("name");
            let message = if name.is_empty() {
                format!("step '{}' has an empty functionRef.name", step.step)
            } else if !functions.is_empty() && !functions.contains(&name) {
                format!(
                    "step '{}' references function '{name}' which is not declared by any loaded Function",
                    step.step
                )
            } else {
                continue;
            };
            out.push(doc.diagnostic(ID, Severity::Error, &path, message));
        }
        out
    }
}
