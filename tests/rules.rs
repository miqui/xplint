//! Pass/fail fixtures for each core rule, plus engine-level behavior.

use std::path::PathBuf;
use xplint::diagnostic::{Diagnostic, Severity};
use xplint::loader::load_tree;
use xplint::rules::Engine;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rules")
        .join(name)
}

/// Lint one fixture file (its documents form the whole corpus) and keep one rule's findings.
fn lint(file: &str, rule: &str) -> Vec<Diagnostic> {
    let corpus = load_tree(&fixture(file)).unwrap();
    Engine::with_core()
        .run(&corpus)
        .into_iter()
        .filter(|d| d.rule == rule)
        .collect()
}

fn summary(d: &[Diagnostic]) -> Vec<(usize, Severity, &str)> {
    d.iter()
        .map(|d| (d.line, d.severity, d.path.as_str()))
        .collect()
}

#[test]
fn xrd_required_without_default() {
    let r = "xrd-required-without-default";
    assert!(lint("xrd-required-without-default.pass.yaml", r).is_empty());
    let d = lint("xrd-required-without-default.fail.yaml", r);
    assert_eq!(
        summary(&d),
        [(
            22,
            Severity::Warning,
            "spec.versions[0].schema.openAPIV3Schema.properties.spec.required[1]"
        )]
    );
    assert!(d[0].message.contains("spec.region"));
}

#[test]
fn composition_missing_function() {
    let r = "composition-missing-function";
    assert!(lint("composition-missing-function.pass.yaml", r).is_empty());
    let d = lint("composition-missing-function.fail.yaml", r);
    assert_eq!(
        summary(&d),
        [(20, Severity::Error, "spec.pipeline[0].functionRef.name")]
    );
    assert!(d[0].message.contains("function-not-installed"));
}

#[test]
fn composition_missing_function_is_silent_without_any_function() {
    // The deletion-policy pass fixture has a Composition but no Function documents.
    let d = lint(
        "composition-missing-deletion-policy.pass.yaml",
        "composition-missing-function",
    );
    assert!(d.is_empty());
}

#[test]
fn composition_missing_deletion_policy() {
    let r = "composition-missing-deletion-policy";
    // pass: one base sets deletionPolicy, the other only managementPolicies.
    assert!(lint("composition-missing-deletion-policy.pass.yaml", r).is_empty());
    let d = lint("composition-missing-deletion-policy.fail.yaml", r);
    assert_eq!(
        summary(&d),
        [(
            22,
            Severity::Warning,
            "spec.pipeline[0].input.resources[0].base.spec"
        )]
    );
}

#[test]
fn patch_path_not_in_xrd() {
    let r = "patch-path-not-in-xrd";
    assert!(lint("patch-path-not-in-xrd.pass.yaml", r).is_empty());
    let d = lint("patch-path-not-in-xrd.fail.yaml", r);
    assert_eq!(
        summary(&d),
        [
            (
                56,
                Severity::Error,
                "spec.pipeline[0].input.resources[0].patches[0].fromFieldPath"
            ),
            (
                60,
                Severity::Error,
                "spec.pipeline[0].input.resources[0].patches[1].toFieldPath"
            ),
        ]
    );
}

#[test]
fn patch_path_rule_skips_when_xrd_not_loaded() {
    assert!(lint(
        "patch-path-not-in-xrd.skipped.yaml",
        "patch-path-not-in-xrd"
    )
    .is_empty());
}

#[test]
fn engine_output_is_sorted_by_file_then_line() {
    let corpus = load_tree(&fixture("patch-path-not-in-xrd.fail.yaml")).unwrap();
    let d = Engine::with_core().run(&corpus);
    assert!(d
        .windows(2)
        .all(|w| (&w[0].file, w[0].line) <= (&w[1].file, w[1].line)));
}
