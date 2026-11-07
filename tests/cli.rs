//! Black-box tests of the `xplint` binary: exit codes, formats, config, ignores, recursion.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// Fresh scratch dir under the cargo target tmp dir.
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-{name}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn xplint(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xplint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}
fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Copy one `examples/violations/<name>` set into a fresh scratch dir as `set/`.
fn with_set(test: &str, set: &str) -> PathBuf {
    let d = scratch(test);
    copy_dir(&examples().join(set), &d.join("set"));
    d
}

const MISSING_FN: &str = "violations/missing-function"; // error, composition.yaml:20
const MISSING_DP: &str = "violations/missing-deletion-policy"; // warning, composition.yaml:22

#[test]
fn clean_set_exits_0_with_no_output() {
    let d = with_set("clean", "clean");
    let o = xplint(&d, &["set"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o), "");
}

#[test]
fn error_finding_exits_1_with_text_output() {
    let d = with_set("error", MISSING_FN);
    let o = xplint(&d, &["set"]);
    assert_eq!(code(&o), 1);
    let s = out(&o);
    assert!(
        s.contains("set/composition.yaml:20: error [composition-missing-function]"),
        "{s}"
    );
    assert!(s.ends_with("1 error(s), 0 warning(s)\n"), "{s}");
}

#[test]
fn warnings_alone_exit_0() {
    let d = with_set("warn", MISSING_DP);
    let o = xplint(&d, &["set"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains("warning [composition-missing-deletion-policy]"));
}

#[test]
fn missing_path_exits_2() {
    let d = scratch("nopath");
    let o = xplint(&d, &["nope.yaml"]);
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("nope.yaml"), "{}", err(&o));
    assert_eq!(out(&o), "");
}

#[test]
fn malformed_yaml_exits_2() {
    let d = scratch("malformed");
    fs::write(d.join("bad.yaml"), "a: [1\nb: 2\n").unwrap();
    let o = xplint(&d, &["bad.yaml"]);
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("bad.yaml"));
}

#[test]
fn usage_errors_exit_2() {
    let d = scratch("usage");
    assert_eq!(code(&xplint(&d, &["--format", "xml"])), 2);
    assert_eq!(code(&xplint(&d, &["--bogus"])), 2);
}

#[test]
fn help_and_version_exit_0() {
    let d = scratch("help");
    let o = xplint(&d, &["--help"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains("--format"));
    assert_eq!(code(&xplint(&d, &["--version"])), 0);
}

#[test]
fn directory_recursion_finds_nested_files_and_defaults_to_cwd() {
    let d = scratch("recurse");
    copy_dir(&examples(), &d.join("deep/er"));
    let o = xplint(&d, &["deep"]);
    assert_eq!(code(&o), 1);
    let s = out(&o);
    for f in [
        "deep/er/violations/missing-function/composition.yaml:20",
        "deep/er/violations/missing-deletion-policy/composition.yaml:22",
        "deep/er/violations/patch-path-not-in-xrd/database.yaml:56",
        "deep/er/violations/required-without-default/xrd.yaml:22",
    ] {
        assert!(s.contains(f), "missing {f} in:\n{s}");
    }
    assert!(!s.contains("clean/"), "{s}");
    assert!(s.ends_with("3 error(s), 2 warning(s)\n"), "{s}");
    // no PATHS: lints cwd
    let o2 = xplint(&d, &[]);
    assert_eq!(out(&o2).lines().count(), s.lines().count());
}

#[test]
fn json_format_is_an_array_of_findings() {
    let d = with_set("json", MISSING_FN);
    let o = xplint(&d, &["set", "--format", "json"]);
    assert_eq!(code(&o), 1);
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["rule"], "composition-missing-function");
    assert_eq!(arr[0]["severity"], "error");
    assert_eq!(arr[0]["file"], "set/composition.yaml");
    assert_eq!(arr[0]["line"], 20);
    assert!(arr[0]["path"].as_str().unwrap().contains("functionRef"));
    assert!(!arr[0]["message"].as_str().unwrap().is_empty());
}

#[test]
fn json_format_clean_is_empty_array() {
    let d = with_set("jsonclean", "clean");
    let o = xplint(&d, &["set", "--format", "json"]);
    assert_eq!(code(&o), 0);
    assert_eq!(out(&o).trim(), "[]");
}

#[test]
fn config_disables_a_rule() {
    let d = with_set("cfg-off", MISSING_FN);
    fs::write(
        d.join("c.yaml"),
        "rules:\n  composition-missing-function: off\n",
    )
    .unwrap();
    let o = xplint(&d, &["set", "--config", "c.yaml"]);
    assert_eq!(code(&o), 0, "{}", out(&o));
    assert_eq!(out(&o), "");
}

#[test]
fn config_overrides_severity_both_ways() {
    let d = with_set("cfg-sev", MISSING_FN);
    fs::write(
        d.join("c.yaml"),
        "rules:\n  composition-missing-function: warning\n",
    )
    .unwrap();
    let o = xplint(&d, &["set", "--config", "c.yaml"]);
    assert_eq!(code(&o), 0);
    assert!(out(&o).contains("warning [composition-missing-function]"));

    let d = with_set("cfg-sev2", MISSING_DP);
    fs::write(
        d.join("c.yaml"),
        "rules:\n  composition-missing-deletion-policy: error\n",
    )
    .unwrap();
    let o = xplint(&d, &["set", "--config", "c.yaml"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).contains("error [composition-missing-deletion-policy]"));
}

#[test]
fn xplint_yaml_in_cwd_is_auto_discovered() {
    let d = with_set("discover", MISSING_FN);
    fs::write(
        d.join(".xplint.yaml"),
        "rules:\n  composition-missing-function: off\n",
    )
    .unwrap();
    assert_eq!(code(&xplint(&d, &["set"])), 0);
    // linting "." also loads .xplint.yaml itself without choking
    assert_eq!(code(&xplint(&d, &["."])), 0);
}

#[test]
fn explicit_config_wins_over_discovered_one() {
    let d = with_set("explicit", MISSING_FN);
    fs::write(
        d.join(".xplint.yaml"),
        "rules:\n  composition-missing-function: off\n",
    )
    .unwrap();
    fs::write(d.join("empty.yaml"), "").unwrap();
    let o = xplint(&d, &["set", "--config", "empty.yaml"]);
    assert_eq!(code(&o), 1);
}

#[test]
fn config_errors_exit_2_with_clear_messages() {
    let d = with_set("cfg-bad", MISSING_FN);
    let cases = [
        (
            "rules:\n  no-such-rule: off\n",
            "unknown rule `no-such-rule`",
        ),
        ("packs:\n  oracle: off\n", "unknown pack `oracle`"),
        ("packs:\n  core: off\n", "`core` pack cannot be toggled"),
        (
            "rules:\n  composition-missing-function: loud\n",
            "invalid config",
        ),
        ("bogus: 1\n", "invalid config"),
    ];
    for (cfg, want) in cases {
        fs::write(d.join("c.yaml"), cfg).unwrap();
        let o = xplint(&d, &["set", "--config", "c.yaml"]);
        assert_eq!(code(&o), 2, "{cfg}");
        assert!(err(&o).contains(want), "{cfg}: {}", err(&o));
        assert!(err(&o).contains("c.yaml"));
        assert_eq!(out(&o), "");
    }
    let o = xplint(&d, &["set", "--config", "missing.yaml"]);
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("missing.yaml"));
}

#[test]
fn known_pack_toggles_are_accepted() {
    let d = with_set("packs-ok", "clean");
    fs::write(
        d.join("c.yaml"),
        "packs:\n  aws: off\n  gcp: on\n  azure: off\n",
    )
    .unwrap();
    assert_eq!(code(&xplint(&d, &["set", "--config", "c.yaml"])), 0);
}

fn read_line_edit(path: &Path, f: impl Fn(Vec<String>) -> Vec<String>) {
    let text = fs::read_to_string(path).unwrap();
    let lines = f(text.lines().map(str::to_string).collect());
    fs::write(path, lines.join("\n") + "\n").unwrap();
}

#[test]
fn trailing_ignore_comment_suppresses_that_line() {
    let d = with_set("ign-line", MISSING_FN);
    read_line_edit(&d.join("set/composition.yaml"), |mut l| {
        l[19].push_str(" # xplint-ignore: composition-missing-function");
        l
    });
    let o = xplint(&d, &["set"]);
    assert_eq!(code(&o), 0, "{}", out(&o));
    assert_eq!(out(&o), "");
}

#[test]
fn comment_line_above_suppresses_next_line() {
    let d = with_set("ign-above", MISSING_FN);
    read_line_edit(&d.join("set/composition.yaml"), |mut l| {
        l.insert(
            19,
            "      # xplint-ignore: composition-missing-function".into(),
        );
        l
    });
    assert_eq!(code(&xplint(&d, &["set"])), 0);
}

#[test]
fn ignore_of_a_different_rule_or_line_does_not_suppress() {
    let d = with_set("ign-other", MISSING_FN);
    read_line_edit(&d.join("set/composition.yaml"), |mut l| {
        l[19].push_str(" # xplint-ignore: some-other-rule");
        l[3].push_str(" # xplint-ignore: composition-missing-function");
        l
    });
    assert_eq!(code(&xplint(&d, &["set"])), 1);
}

#[test]
fn document_header_ignore_suppresses_whole_document() {
    // composition.yaml = Function doc, `---` on line 7, then the Composition doc.
    let d = with_set("ign-doc", MISSING_FN);
    read_line_edit(&d.join("set/composition.yaml"), |mut l| {
        l.insert(7, "# xplint-ignore: composition-missing-function".into());
        l
    });
    let o = xplint(&d, &["set"]);
    assert_eq!(code(&o), 0, "{}", out(&o));
}

#[test]
fn document_ignore_is_scoped_to_its_own_document() {
    let d = with_set("ign-scope", MISSING_FN);
    read_line_edit(&d.join("set/composition.yaml"), |mut l| {
        let composition: Vec<String> = l[7..].to_vec();
        l.insert(7, "# xplint-ignore: composition-missing-function".into());
        l.push("---".into());
        l.extend(composition);
        l
    });
    let o = xplint(&d, &["set"]);
    let s = out(&o);
    let hits: Vec<&str> = s
        .lines()
        .filter(|l| l.contains("[composition-missing-function]"))
        .collect();
    assert_eq!(hits.len(), 1, "{s}");
    let line: usize = hits[0].split(':').nth(1).unwrap().parse().unwrap();
    assert!(line > 30, "finding should be in the 2nd Composition: {s}");
    assert_eq!(code(&o), 1);
}
