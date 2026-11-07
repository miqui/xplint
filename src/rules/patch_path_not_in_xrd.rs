//! `patch-path-not-in-xrd`: patch field paths on the composite that the XRD schema lacks.
//!
//! For each Composition, the XRD is found via `compositeTypeRef` among the loaded
//! documents (the rule is skipped when it is not loaded). Only the composite
//! (XR) side of a patch is checked, since the composed resource schema is not
//! known: `fromFieldPath` for `FromCompositeFieldPath` (the default) and
//! `CombineFromComposite`, `toFieldPath` for `ToCompositeFieldPath` and
//! `CombineToComposite`. Patches are read from `input.resources[*].patches`
//! and `input.patchSets[*].patches` of function-patch-and-transform steps.
//!
//! Schema walking: `properties` entries must exist; `additionalProperties`
//! schemas and `x-kubernetes-preserve-unknown-fields` allow anything below;
//! an object schema without `properties` is treated as free-form. `apiVersion`,
//! `kind`, `metadata.*`, Crossplane machinery under `spec` (e.g.
//! `compositionRef`, `crossplane`, `writeConnectionSecretToRef`) and
//! `status.conditions` are always allowed. Paths using `[*]` are skipped.

use super::{Doc, Rule};
use crate::diagnostic::{Diagnostic, Severity};
use crate::model::Xrd;
use crate::yamlpath::{parse_field_path, Seg, YamlPath};
use serde_yaml_ng::Value;

pub struct PatchPathNotInXrd;

pub const ID: &str = "patch-path-not-in-xrd";

/// Fields Crossplane injects under `spec` of every composite resource.
const SPEC_MACHINERY: &[&str] = &[
    "compositionRef",
    "compositionSelector",
    "compositionRevisionRef",
    "compositionRevisionSelector",
    "compositionUpdatePolicy",
    "claimRef",
    "environmentConfigRefs",
    "resourceRefs",
    "writeConnectionSecretToRef",
    "publishConnectionDetailsTo",
    "crossplane",
];

impl Rule for PatchPathNotInXrd {
    fn id(&self) -> &'static str {
        ID
    }
    fn description(&self) -> &'static str {
        "Patch fromFieldPath/toFieldPath is not defined in the XRD schema"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
        let Some(comp) = doc.composition() else {
            return Vec::new();
        };
        let type_ref = &comp.spec.composite_type_ref;
        let Some(xrd) = doc.find_xrd(&type_ref.api_version, &type_ref.kind) else {
            return Vec::new();
        };
        let (_, version) = crate::model::split_api_version(&type_ref.api_version);
        let Some(schema) = version_schema(xrd, version) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (i, step) in comp.spec.pipeline.iter().enumerate() {
            let Some(input) = step.input.as_ref() else {
                continue;
            };
            let base = YamlPath::new()
                .key("spec")
                .key("pipeline")
                .idx(i)
                .key("input");
            let ctx = Ctx { doc, schema };
            if let Some(Value::Sequence(res)) = input.get("resources") {
                for (j, r) in res.iter().enumerate() {
                    let p = base.key("resources").idx(j).key("patches");
                    ctx.patches(r.get("patches"), &p, &mut out);
                }
            }
            if let Some(Value::Sequence(sets)) = input.get("patchSets") {
                for (j, s) in sets.iter().enumerate() {
                    let p = base.key("patchSets").idx(j).key("patches");
                    ctx.patches(s.get("patches"), &p, &mut out);
                }
            }
        }
        out
    }
}

fn version_schema<'a>(xrd: &'a Xrd, version: &str) -> Option<&'a Value> {
    xrd.spec
        .versions
        .iter()
        .find(|v| v.name == version)?
        .schema
        .as_ref()?
        .open_api_v3_schema
        .as_ref()
}

struct Ctx<'a, 'd> {
    doc: &'a Doc<'d>,
    schema: &'a Value,
}

impl Ctx<'_, '_> {
    fn patches(&self, patches: Option<&Value>, path: &YamlPath, out: &mut Vec<Diagnostic>) {
        let Some(Value::Sequence(list)) = patches else {
            return;
        };
        for (k, patch) in list.iter().enumerate() {
            let p = path.idx(k);
            let kind = patch
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("FromCompositeFieldPath");
            match kind {
                "FromCompositeFieldPath" => self.check(patch, "fromFieldPath", &p, out),
                "ToCompositeFieldPath" => self.check(patch, "toFieldPath", &p, out),
                "CombineFromComposite" => {
                    if let Some(Value::Sequence(vars)) =
                        patch.get("combine").and_then(|c| c.get("variables"))
                    {
                        let vp = p.key("combine").key("variables");
                        for (m, var) in vars.iter().enumerate() {
                            self.check(var, "fromFieldPath", &vp.idx(m), out);
                        }
                    }
                }
                "CombineToComposite" => self.check(patch, "toFieldPath", &p, out),
                _ => {}
            }
        }
    }

    fn check(&self, node: &Value, field: &str, path: &YamlPath, out: &mut Vec<Diagnostic>) {
        let Some(raw) = node.get(field).and_then(Value::as_str) else {
            return;
        };
        let Some(segs) = parse_field_path(raw) else {
            return;
        };
        if !xr_path_exists(self.schema, &segs) {
            out.push(self.doc.diagnostic(
                ID,
                Severity::Error,
                &path.key(field),
                format!("path '{raw}' is not defined in the XRD schema"),
            ));
        }
    }
}

/// Does `segs` (rooted at the composite resource) exist per the XRD schema?
fn xr_path_exists(root: &Value, segs: &[Seg]) -> bool {
    match segs.first() {
        Some(Seg::Key(k)) if matches!(k.as_str(), "apiVersion" | "kind" | "metadata") => true,
        Some(Seg::Key(k)) if k == "spec" => match segs.get(1) {
            Some(Seg::Key(f)) if SPEC_MACHINERY.contains(&f.as_str()) => true,
            _ => schema_has(root, segs),
        },
        Some(Seg::Key(k)) if k == "status" => match segs.get(1) {
            Some(Seg::Key(f)) if f == "conditions" => true,
            _ => schema_has(root, segs),
        },
        _ => false,
    }
}

fn schema_has(schema: &Value, segs: &[Seg]) -> bool {
    if schema.get("x-kubernetes-preserve-unknown-fields") == Some(&Value::Bool(true)) {
        return true;
    }
    let ty = schema.get("type").and_then(Value::as_str);
    match segs.first() {
        None => true,
        Some(Seg::Key(k)) => {
            if matches!(
                ty,
                Some("string" | "integer" | "number" | "boolean" | "array")
            ) {
                return false;
            }
            if let Some(child) = schema.get("properties").and_then(|p| p.get(k.as_str())) {
                return schema_has(child, &segs[1..]);
            }
            match schema.get("additionalProperties") {
                Some(Value::Bool(true)) => true,
                Some(m @ Value::Mapping(_)) => schema_has(m, &segs[1..]),
                _ => schema.get("properties").is_none(),
            }
        }
        Some(Seg::Idx(_)) => match (ty, schema.get("items")) {
            (Some("array") | None, Some(items)) => schema_has(items, &segs[1..]),
            (Some("array") | None, None) => true,
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        serde_yaml_ng::from_str(
            "type: object
properties:
  spec:
    type: object
    properties:
      size: {type: string}
      tags: {type: object, additionalProperties: {type: string}}
      free: {type: object, x-kubernetes-preserve-unknown-fields: true}
      loose: {type: object}
      items:
        type: array
        items:
          type: object
          properties:
            name: {type: string}
",
        )
        .unwrap()
    }

    fn ok(p: &str) -> bool {
        xr_path_exists(&schema(), &parse_field_path(p).unwrap())
    }

    #[test]
    fn resolves_against_schema() {
        assert!(ok("spec.size"));
        assert!(!ok("spec.sizee"));
        assert!(!ok("spec.size.deeper"));
        assert!(ok("spec.tags.anything"));
        assert!(ok("spec.free.a.b"));
        assert!(ok("spec.loose.a"));
        assert!(ok("spec.items[0].name"));
        assert!(!ok("spec.items[0].nope"));
        assert!(!ok("spec.items.name"));
        assert!(!ok("spec.size[0]"));
    }

    #[test]
    fn always_allowed_and_unknown_roots() {
        assert!(ok("metadata.labels[\"a.b\"]"));
        assert!(ok("metadata.name"));
        assert!(ok("spec.compositionRef.name"));
        assert!(ok("status.conditions"));
        assert!(!ok("status.endpoint"));
        assert!(!ok("data.x"));
    }
}
