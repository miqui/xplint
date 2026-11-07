//! End-to-end pack acceptance cases (a)-(c) through the engine and config
//! pipeline, parametrized over AWS, GCP and Azure (see tests/common).
//!
//! No provider rules ship yet (Stage 5), so test-registered rules stand in.
//! Static YAML only: no cloud, cluster or provider CLI is involved.

mod common;
use common::{config, corpus, engine_all, lint, PROVIDERS, XRD};
use xplint::diagnostic::Severity;
use xplint::run::lint_corpus;

fn ids(yaml: &str, cfg: Option<&str>) -> Vec<String> {
    lint(cfg, yaml).into_iter().map(|d| d.rule).collect()
}

#[test]
fn a_pack_auto_activates_with_no_config() {
    for p in &PROVIDERS {
        assert_eq!(ids(&p.doc(), None), [p.rule_id], "{}", p.name);
    }
}

#[test]
fn b_pack_off_yields_no_findings_for_that_provider_only() {
    for p in &PROVIDERS {
        let cfg = format!("packs:\n  {}: off\n", p.name);
        assert!(ids(&p.doc(), Some(&cfg)).is_empty(), "{}", p.name);
        // other providers' documents are unaffected
        for q in PROVIDERS.iter().filter(|q| q.name != p.name) {
            assert_eq!(ids(&q.doc(), Some(&cfg)), [q.rule_id]);
        }
    }
}

#[test]
fn c_pack_on_does_not_force_activation_on_other_docs() {
    for p in &PROVIDERS {
        let cfg = format!("packs:\n  {}: on\n", p.name);
        assert!(ids(XRD, Some(&cfg)).is_empty(), "{}", p.name);
        assert_eq!(ids(&p.doc(), Some(&cfg)), [p.rule_id], "{}", p.name);
    }
}

#[test]
fn providers_do_not_cross_activate() {
    for p in &PROVIDERS {
        // right group, wrong kind of another provider
        let swapped = format!("apiVersion: {}\nkind: Nope\n", p.api_version);
        assert!(ids(&swapped, None).is_empty(), "{}", p.name);
    }
}

#[test]
fn pack_rules_can_be_disabled_or_downgraded_individually() {
    for p in &PROVIDERS {
        let off = format!("rules:\n  {}: off\n", p.rule_id);
        assert!(ids(&p.doc(), Some(&off)).is_empty(), "{}", p.name);
        let warn = config(&format!("rules:\n  {}: warning\n", p.rule_id));
        let d = lint_corpus(engine_all(), Some(&warn), &corpus("t.yaml", &p.doc())).unwrap();
        assert_eq!(d[0].severity, Severity::Warning, "{}", p.name);
    }
}
