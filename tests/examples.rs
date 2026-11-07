//! Lints the shipped `examples/` tree and asserts the findings documented in examples/README.md.

use std::path::Path;
use xplint::loader::load_tree;
use xplint::rules::Engine;

#[test]
fn examples_produce_exactly_the_documented_findings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let corpus = load_tree(&root).unwrap();
    let got: Vec<(String, usize, String, String)> = Engine::with_core()
        .run(&corpus)
        .into_iter()
        .map(|d| {
            let rel = d.file.strip_prefix(&root).unwrap().display().to_string();
            (rel, d.line, d.severity.to_string(), d.rule)
        })
        .collect();
    let want = [
        (
            "violations/missing-deletion-policy/composition.yaml",
            22,
            "warning",
            "composition-missing-deletion-policy",
        ),
        (
            "violations/missing-function/composition.yaml",
            20,
            "error",
            "composition-missing-function",
        ),
        (
            "violations/patch-path-not-in-xrd/database.yaml",
            56,
            "error",
            "patch-path-not-in-xrd",
        ),
        (
            "violations/patch-path-not-in-xrd/database.yaml",
            60,
            "error",
            "patch-path-not-in-xrd",
        ),
        (
            "violations/required-without-default/xrd.yaml",
            22,
            "warning",
            "xrd-required-without-default",
        ),
    ]
    .map(|(f, l, s, r)| (f.to_string(), l, s.to_string(), r.to_string()));
    assert_eq!(got, want);
    // Nothing is reported for the clean set.
    assert!(got.iter().all(|g| !g.0.starts_with("clean/")));
}
