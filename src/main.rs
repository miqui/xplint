use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use xplint::rules::Engine;
use xplint::run::{
    has_errors, lint_corpus, load_config, load_paths, render_json, render_text, RunError,
};
use xplint::sarif;

/// Opinionated linter for Crossplane XRDs, Compositions and Functions.
///
/// Exit codes: 0 clean (warnings alone are clean), 1 findings at error
/// level, 2 usage or IO/config error.
#[derive(Parser, Debug)]
#[command(name = "xplint", version, about)]
struct Cli {
    /// Files or directories to lint (directories are searched recursively
    /// for .yaml/.yml). Defaults to the current directory.
    paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Config file. Without it, ./.xplint.yaml is used if present.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
    Sarif,
}

/// Returns whether any error-level finding was reported.
fn run(cli: Cli) -> Result<bool, RunError> {
    let config = load_config(cli.config.as_deref(), Path::new("."))?;
    let paths = if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths
    };
    let corpus = load_paths(&paths)?;
    let engine = Engine::with_core();
    let rules = engine.rule_metadata();
    let diagnostics = lint_corpus(engine, config.as_ref(), &corpus)?;
    let rendered = match cli.format {
        Format::Text => render_text(&diagnostics),
        Format::Json => render_json(&diagnostics),
        Format::Sarif => sarif::render(&diagnostics, &rules),
    };
    print!("{rendered}");
    Ok(has_errors(&diagnostics))
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(1),
        Err(e) => {
            eprintln!("xplint: error: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_paths_format_and_config() {
        let cli =
            Cli::try_parse_from(["xplint", "a", "b", "--format", "json", "--config", "c.yaml"])
                .unwrap();
        assert_eq!(cli.paths.len(), 2);
        assert!(matches!(cli.format, Format::Json));
        assert_eq!(cli.config.as_deref(), Some(Path::new("c.yaml")));
    }

    #[test]
    fn parses_sarif_format() {
        let cli = Cli::try_parse_from(["xplint", "--format", "sarif"]).unwrap();
        assert!(matches!(cli.format, Format::Sarif));
        assert!(Cli::try_parse_from(["xplint", "--format", "xml"]).is_err());
    }
}
