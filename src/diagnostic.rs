//! Diagnostics produced by rules.

use std::fmt;
use std::path::PathBuf;

/// Severity of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// A located finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Rule id, e.g. `xrd-required-without-default`.
    pub rule: String,
    pub severity: Severity,
    pub file: PathBuf,
    /// 1-based line in `file`.
    pub line: usize,
    /// YAML path of the offending node, e.g. `spec.pipeline[0].functionRef.name`.
    pub path: String,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} [{}] {} ({})",
            self.file.display(),
            self.line,
            self.severity,
            self.rule,
            self.message,
            self.path
        )
    }
}
