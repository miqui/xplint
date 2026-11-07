//! SARIF 2.1.0 output (`--format sarif`) for GitHub code scanning.
//!
//! One run, tool `xplint`; `tool.driver.rules` lists every registered rule
//! (not just those that fired) so `ruleIndex` is always resolvable.

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::RuleMeta;
use serde::Serialize;
use std::path::Path;

pub const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
pub const SARIF_VERSION: &str = "2.1.0";
pub const INFORMATION_URI: &str = "https://github.com/miqui/xplint";

#[derive(Debug, Serialize)]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub version: &'static str,
    pub runs: Vec<Run>,
}

#[derive(Debug, Serialize)]
pub struct Run {
    pub tool: Tool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub driver: Driver,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    pub name: &'static str,
    pub version: &'static str,
    pub information_uri: &'static str,
    pub rules: Vec<ReportingDescriptor>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportingDescriptor {
    pub id: String,
    pub short_description: Text,
    pub default_configuration: Configuration,
}

#[derive(Debug, Serialize)]
pub struct Text {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Configuration {
    pub level: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    /// Omitted only if the rule is not in `driver.rules` (cannot happen for engine output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<usize>,
    pub level: &'static str,
    pub message: Text,
    pub locations: Vec<Location>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub physical_location: PhysicalLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalLocation {
    pub artifact_location: ArtifactLocation,
    pub region: Region,
}

#[derive(Debug, Serialize)]
pub struct ArtifactLocation {
    pub uri: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub start_line: usize,
}

/// SARIF level for a severity.
pub fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// Relative, forward-slash, percent-encoded URI reference for a path.
/// A leading `./` is dropped; an absolute path becomes a `file://` URI.
pub fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let (prefix, rest) = if s.starts_with('/') {
        ("file://", s.as_str())
    } else {
        ("", s.trim_start_matches("./"))
    };
    let mut out = String::from(prefix);
    for b in rest.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build the SARIF log for `diagnostics`, with `rules` as the driver's rule list.
pub fn build(diagnostics: &[Diagnostic], rules: &[RuleMeta]) -> SarifLog {
    let descriptors = rules
        .iter()
        .map(|r| ReportingDescriptor {
            id: r.id.to_string(),
            short_description: Text {
                text: r.description.to_string(),
            },
            default_configuration: Configuration {
                level: level(r.severity),
            },
        })
        .collect();
    let results = diagnostics
        .iter()
        .map(|d| SarifResult {
            rule_id: d.rule.clone(),
            rule_index: rules.iter().position(|r| r.id == d.rule),
            level: level(d.severity),
            message: Text {
                text: d.message.clone(),
            },
            locations: vec![Location {
                physical_location: PhysicalLocation {
                    artifact_location: ArtifactLocation {
                        uri: path_to_uri(&d.file),
                    },
                    region: Region { start_line: d.line },
                },
            }],
        })
        .collect();
    SarifLog {
        schema: SARIF_SCHEMA,
        version: SARIF_VERSION,
        runs: vec![Run {
            tool: Tool {
                driver: Driver {
                    name: "xplint",
                    version: env!("CARGO_PKG_VERSION"),
                    information_uri: INFORMATION_URI,
                    rules: descriptors,
                },
            },
            results,
        }],
    }
}

/// Pretty-printed SARIF JSON with a trailing newline.
pub fn render(diagnostics: &[Diagnostic], rules: &[RuleMeta]) -> String {
    let mut s = serde_json::to_string_pretty(&build(diagnostics, rules)).expect("sarif serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn uri_is_relative_forward_slash_and_encoded() {
        assert_eq!(path_to_uri(&PathBuf::from("./a/b.yaml")), "a/b.yaml");
        assert_eq!(path_to_uri(&PathBuf::from("a\\b c.yaml")), "a/b%20c.yaml");
        assert_eq!(path_to_uri(&PathBuf::from("/x/y.yaml")), "file:///x/y.yaml");
    }
}
