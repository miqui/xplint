//! End-to-end pack acceptance cases (a)-(c) through the engine and config pipeline.
//!
//! No AWS rules ship yet (Stage 5), so a test-registered rule `test-aws-instance`
//! (pack `aws`, kind `Instance`) stands in. The real binary registers only core
//! rules; `lint_corpus` takes the engine so tests can add rules.

use std::path::{Path, PathBuf};
use xplint::config::Config;
use xplint::diagnostic::{Diagnostic, Severity};
use xplint::loader::{load_str, SourceDoc};
use xplint::rules::{Doc, Engine, Pack, Rule};
use xplint::run::lint_corpus;
use xplint::yamlpath::YamlPath;

struct TestAws;

impl Rule for TestAws {
    fn id(&self) -> &'static str {
        "test-aws-instance"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn pack(&self) -> Pack {
        Pack::Aws
    }
    fn kinds(&self) -> &'static [&'static str] {
        &["Instance"]
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
        vec![doc.diagnostic(
            self.id(),
            self.severity(),
            &YamlPath::new(),
            "aws hit".into(),
        )]
    }
}

const RDS: &str = "apiVersion: rds.aws.upbound.io/v1beta1\nkind: Instance\n";
const XRD: &str = "apiVersion: apiextensions.crossplane.io/v1\nkind: CompositeResourceDefinition\nspec: {group: g.example.org, names: {kind: K, plural: ks}}\n";

fn corpus(yaml: &str) -> Vec<SourceDoc> {
    load_str(Path::new("t.yaml"), yaml)
        .unwrap()
        .into_iter()
        .map(|document| SourceDoc {
            file: PathBuf::from("t.yaml"),
            document,
        })
        .collect()
}

fn engine() -> Engine {
    let mut e = Engine::with_core();
    e.register(Box::new(TestAws));
    e
}

fn lint(config: Option<&str>, yaml: &str) -> Vec<String> {
    let cfg = config.map(|c| {
        (
            PathBuf::from(".xplint.yaml"),
            Config::parse(Path::new(".xplint.yaml"), c).unwrap(),
        )
    });
    lint_corpus(engine(), cfg.as_ref(), &corpus(yaml))
        .unwrap()
        .into_iter()
        .map(|d| d.rule)
        .collect()
}

#[test]
fn a_aws_pack_auto_activates_with_no_config() {
    assert_eq!(lint(None, RDS), ["test-aws-instance"]);
}

#[test]
fn b_packs_aws_off_yields_no_aws_findings() {
    assert!(lint(Some("packs:\n  aws: off\n"), RDS).is_empty());
}

#[test]
fn c_packs_aws_on_does_not_force_activation_on_other_docs() {
    assert!(lint(Some("packs:\n  aws: on\n"), XRD).is_empty());
    // and `on` on a matching doc behaves like the default
    assert_eq!(
        lint(Some("packs:\n  aws: on\n"), RDS),
        ["test-aws-instance"]
    );
}

#[test]
fn pack_rules_can_be_disabled_or_downgraded_individually() {
    assert!(lint(Some("rules:\n  test-aws-instance: off\n"), RDS).is_empty());
    let cfg = Config::parse(Path::new("c"), "rules:\n  test-aws-instance: warning\n").unwrap();
    let d = lint_corpus(engine(), Some(&(PathBuf::from("c"), cfg)), &corpus(RDS)).unwrap();
    assert_eq!(d[0].severity, Severity::Warning);
}
