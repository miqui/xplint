//! The lint pipeline behind the CLI: load, run rules, apply config and ignores, render.

use crate::config::{Config, ConfigError, DEFAULT_CONFIG_FILE};
use crate::diagnostic::{Diagnostic, Severity};
use crate::ignore::filter_ignored;
use crate::loader::{load_tree, LoadError, SourceDoc};
use crate::rules::Engine;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Anything that maps to exit code 2.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error(transparent)]
    Config(#[from] ConfigError),
}

/// A loaded config and the file it came from (used in error messages).
pub type LoadedConfig = (PathBuf, Config);

/// Load `explicit` if given (missing file is an error); otherwise
/// `<dir>/.xplint.yaml` if it exists; otherwise no config.
pub fn load_config(
    explicit: Option<&Path>,
    dir: &Path,
) -> Result<Option<LoadedConfig>, ConfigError> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            let p = dir.join(DEFAULT_CONFIG_FILE);
            if !p.is_file() {
                return Ok(None);
            }
            p
        }
    };
    let cfg = Config::load(&path)?;
    Ok(Some((path, cfg)))
}

/// Load every path (files or directories, recursively) into one corpus.
pub fn load_paths(paths: &[PathBuf]) -> Result<Vec<SourceDoc>, LoadError> {
    let mut corpus = Vec::new();
    for p in paths {
        corpus.extend(load_tree(p)?);
    }
    Ok(corpus)
}

/// Apply `config` to `engine`, run it over `corpus`, then apply rule
/// overrides and inline ignores. Takes the engine so tests can register extra rules.
pub fn lint_corpus(
    mut engine: Engine,
    config: Option<&LoadedConfig>,
    corpus: &[SourceDoc],
) -> Result<Vec<Diagnostic>, ConfigError> {
    if let Some((file, cfg)) = config {
        cfg.apply(file, &mut engine)?;
    }
    let mut diagnostics = engine.run(corpus);
    if let Some((_, cfg)) = config {
        diagnostics = cfg.postprocess(diagnostics);
    }
    Ok(filter_ignored(diagnostics))
}

/// Exit code for a result set: 1 if any error-level finding, else 0.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

pub fn render_text(diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    for d in diagnostics {
        out.push_str(&d.to_string());
        out.push('\n');
    }
    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diagnostics.len() - errors;
    if !diagnostics.is_empty() {
        out.push_str(&format!("{errors} error(s), {warnings} warning(s)\n"));
    }
    out
}

pub fn render_json(diagnostics: &[Diagnostic]) -> String {
    let items: Vec<_> = diagnostics
        .iter()
        .map(|d| {
            json!({
                "rule": d.rule,
                "severity": d.severity.to_string(),
                "file": d.file.display().to_string(),
                "line": d.line,
                "path": d.path,
                "message": d.message,
            })
        })
        .collect();
    let mut s = serde_json::to_string_pretty(&items).expect("diagnostics serialize");
    s.push('\n');
    s
}
