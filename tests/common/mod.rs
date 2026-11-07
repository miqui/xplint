//! Shared provider-generic helpers. Everything here is static YAML plus
//! test-registered rules: no cloud, cluster or provider CLI is ever touched.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use xplint::config::Config;
use xplint::diagnostic::{Diagnostic, Severity};
use xplint::loader::{load_str, SourceDoc};
use xplint::rules::{Doc, Engine, Pack, Rule};
use xplint::run::lint_corpus;
use xplint::yamlpath::YamlPath;

/// One provider under test. Every pack/SARIF test runs over all of `PROVIDERS`.
pub struct Provider {
    pub name: &'static str,
    pub pack: Pack,
    pub api_version: &'static str,
    pub kind: &'static str,
    pub rule_id: &'static str,
}

pub const PROVIDERS: [Provider; 3] = [
    Provider {
        name: "aws",
        pack: Pack::Aws,
        api_version: "rds.aws.upbound.io/v1beta1",
        kind: "Instance",
        rule_id: "test-aws-instance",
    },
    Provider {
        name: "gcp",
        pack: Pack::Gcp,
        api_version: "sql.gcp.upbound.io/v1beta1",
        kind: "DatabaseInstance",
        rule_id: "test-gcp-databaseinstance",
    },
    Provider {
        name: "azure",
        pack: Pack::Azure,
        api_version: "dbforpostgresql.azure.upbound.io/v1beta1",
        kind: "FlexibleServer",
        rule_id: "test-azure-flexibleserver",
    },
];

impl Provider {
    pub fn doc(&self) -> String {
        format!("apiVersion: {}\nkind: {}\n", self.api_version, self.kind)
    }

    pub fn rule(&self) -> TestPackRule {
        TestPackRule {
            id: self.rule_id,
            pack: self.pack,
            kind: [self.kind],
        }
    }
}

pub struct TestPackRule {
    id: &'static str,
    pack: Pack,
    kind: [&'static str; 1],
}

impl Rule for TestPackRule {
    fn id(&self) -> &'static str {
        self.id
    }
    fn description(&self) -> &'static str {
        "test-registered pack rule"
    }
    fn severity(&self) -> Severity {
        Severity::Error
    }
    fn pack(&self) -> Pack {
        self.pack
    }
    fn kinds(&self) -> &'static [&'static str] {
        // Leak-free: kinds are 'static strs from PROVIDERS; box a slice once per call.
        Box::leak(Box::new(self.kind))
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
        vec![doc.diagnostic(
            self.id,
            self.severity(),
            &YamlPath::new(),
            "pack hit".into(),
        )]
    }
}

pub const XRD: &str = "apiVersion: apiextensions.crossplane.io/v1\nkind: CompositeResourceDefinition\nspec: {group: g.example.org, names: {kind: K, plural: ks}}\n";

pub fn corpus(file: &str, yaml: &str) -> Vec<SourceDoc> {
    load_str(Path::new(file), yaml)
        .unwrap()
        .into_iter()
        .map(|document| SourceDoc {
            file: PathBuf::from(file),
            document,
        })
        .collect()
}

/// Core rules plus the test rules of every provider.
pub fn engine_all() -> Engine {
    let mut e = Engine::with_core();
    for p in &PROVIDERS {
        e.register(Box::new(p.rule()));
    }
    e
}

pub fn config(text: &str) -> (PathBuf, Config) {
    let p = PathBuf::from(".xplint.yaml");
    let c = Config::parse(&p, text).unwrap();
    (p, c)
}

pub fn lint(config_text: Option<&str>, yaml: &str) -> Vec<Diagnostic> {
    let cfg = config_text.map(config);
    lint_corpus(engine_all(), cfg.as_ref(), &corpus("t.yaml", yaml)).unwrap()
}
