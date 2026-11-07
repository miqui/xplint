//! SARIF output: deterministic snapshots plus validation against the vendored
//! official SARIF 2.1.0 JSON schema (see tests/fixtures/README.md).
//! Regenerate snapshots with `UPDATE_SNAPSHOTS=1 cargo test --test sarif`.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> PathBuf {
    root().join("tests/fixtures").join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xplint"))
        .current_dir(root())
        .args(args)
        .args(["--format", "sarif"])
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn schema_validator() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(&fs::read_to_string(fixture("sarif-schema-2.1.0.json")).unwrap())
            .unwrap();
    jsonschema::draft4::new(&schema).expect("SARIF schema compiles")
}

fn assert_valid(doc: &str) {
    let v: Value = serde_json::from_str(doc).expect("output is JSON");
    let errors: Vec<String> = schema_validator()
        .iter_errors(&v)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
}

/// Compare against a checked-in snapshot; the crate version is masked so a
/// version bump does not churn the fixture.
fn assert_snapshot(actual: &str, name: &str) {
    let masked = actual.replace(
        &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
        "\"version\": \"@VERSION@\"",
    );
    let path = fixture(name);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::write(&path, &masked).unwrap();
    }
    let expected = fs::read_to_string(&path).expect("snapshot exists");
    assert_eq!(masked, expected, "snapshot {name} differs");
}

#[test]
fn examples_snapshot_and_exit_code() {
    let o = run(&["examples"]);
    assert_eq!(o.status.code(), Some(1)); // exit codes unchanged: error findings -> 1
    let s = stdout(&o);
    assert_valid(&s);
    assert_snapshot(&s, "sarif-examples.expected.json");
}

#[test]
fn clean_run_is_valid_with_empty_results() {
    let o = run(&["examples/clean"]);
    assert_eq!(o.status.code(), Some(0));
    let s = stdout(&o);
    assert_valid(&s);
    let v: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 0);
    let rules = v["runs"][0]["tool"]["driver"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 4);
    assert_snapshot(&s, "sarif-clean.expected.json");
}

#[test]
fn structure_matches_spec() {
    let o = run(&["examples"]);
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["version"], "2.1.0");
    assert_eq!(v["runs"].as_array().unwrap().len(), 1);
    let driver = &v["runs"][0]["tool"]["driver"];
    assert_eq!(driver["name"], "xplint");
    assert_eq!(driver["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(driver["informationUri"], "https://github.com/miqui/xplint");
    let rules = driver["rules"].as_array().unwrap();
    for r in rules {
        assert!(r["id"].is_string());
        assert!(r["shortDescription"]["text"].as_str().unwrap().len() > 5);
        let level = r["defaultConfiguration"]["level"].as_str().unwrap();
        assert!(["error", "warning", "note"].contains(&level));
    }
    for res in v["runs"][0]["results"].as_array().unwrap() {
        let idx = res["ruleIndex"].as_u64().unwrap() as usize;
        assert_eq!(rules[idx]["id"], res["ruleId"]);
        let loc = &res["locations"][0]["physicalLocation"];
        let uri = loc["artifactLocation"]["uri"].as_str().unwrap();
        assert!(uri.starts_with("examples/") && !uri.contains('\\'));
        assert!(loc["region"]["startLine"].as_u64().unwrap() >= 1);
    }
}

#[test]
fn schema_rejects_a_broken_document() {
    // Guards against the validator silently accepting everything.
    let bad = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{}},"results":[]}]}"#;
    let v: Value = serde_json::from_str(bad).unwrap();
    assert!(!schema_validator().is_valid(&v));
}

#[test]
fn config_severity_override_changes_result_level_not_default() {
    let o = run(&[
        "examples",
        "--config",
        "tests/fixtures/sarif-override.xplint.yaml",
    ]);
    let s = stdout(&o);
    assert_valid(&s);
    let v: Value = serde_json::from_str(&s).unwrap();
    let run0 = &v["runs"][0];
    let rule = run0["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "composition-missing-function")
        .unwrap();
    assert_eq!(rule["defaultConfiguration"]["level"], "error");
    let res = run0["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ruleId"] == "composition-missing-function")
        .unwrap();
    assert_eq!(res["level"], "warning");
}

mod common;

/// SARIF for pack rules across AWS, GCP and Azure documents, through the
/// library pipeline (the binary ships core rules only). Static YAML; no cloud access.
#[test]
fn pack_rules_for_every_provider_serialize_and_validate() {
    use common::{corpus, engine_all, PROVIDERS};
    let yaml: String = PROVIDERS
        .iter()
        .map(|p| p.doc())
        .collect::<Vec<_>>()
        .join("---\n");
    let corpus = corpus("providers.yaml", &yaml);
    let engine = engine_all();
    let rules = engine.rule_metadata();
    let diags = xplint::run::lint_corpus(engine, None, &corpus).unwrap();
    assert_eq!(diags.len(), PROVIDERS.len());
    let s = xplint::sarif::render(&diags, &rules);
    assert_valid(&s);
    let v: Value = serde_json::from_str(&s).unwrap();
    let results = v["runs"][0]["results"].as_array().unwrap();
    let driver_rules = v["runs"][0]["tool"]["driver"]["rules"].as_array().unwrap();
    for p in &PROVIDERS {
        let r = results
            .iter()
            .find(|r| r["ruleId"] == p.rule_id)
            .unwrap_or_else(|| panic!("no result for {}", p.name));
        let idx = r["ruleIndex"].as_u64().unwrap() as usize;
        assert_eq!(driver_rules[idx]["id"], p.rule_id);
        assert_eq!(
            r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "providers.yaml"
        );
    }
}
