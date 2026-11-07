//! `.xplint.yaml` configuration: per-rule overrides and pack toggles.

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::{Engine, Pack};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// File name auto-discovered in the current directory.
pub const DEFAULT_CONFIG_FILE: &str = ".xplint.yaml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{}: {source}", file.display())]
    Io {
        file: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}: invalid config: {message}", file.display())]
    Parse { file: PathBuf, message: String },
    #[error("{}: {message}", file.display())]
    Invalid { file: PathBuf, message: String },
}

/// On/off switch, accepts `on`/`off` and YAML booleans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    On,
    Off,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Raw {
    Bool(bool),
    Str(String),
}

impl<'de> Deserialize<'de> for Toggle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Raw::deserialize(d)? {
            Raw::Bool(true) => Ok(Toggle::On),
            Raw::Bool(false) => Ok(Toggle::Off),
            Raw::Str(s) if s == "on" => Ok(Toggle::On),
            Raw::Str(s) if s == "off" => Ok(Toggle::Off),
            Raw::Str(s) => Err(serde::de::Error::custom(format!(
                "expected `on` or `off`, got `{s}`"
            ))),
        }
    }
}

/// Per-rule setting: `off`, or a severity override (`warning` / `error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSetting {
    Off,
    Severity(Severity),
}

impl<'de> Deserialize<'de> for RuleSetting {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Raw::deserialize(d)? {
            Raw::Bool(false) => Ok(RuleSetting::Off),
            Raw::Str(s) => match s.as_str() {
                "off" => Ok(RuleSetting::Off),
                "warning" => Ok(RuleSetting::Severity(Severity::Warning)),
                "error" => Ok(RuleSetting::Severity(Severity::Error)),
                _ => Err(serde::de::Error::custom(format!(
                    "expected `off`, `warning` or `error`, got `{s}`"
                ))),
            },
            Raw::Bool(true) => Err(serde::de::Error::custom(
                "expected `off`, `warning` or `error`, got `true`",
            )),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub rules: BTreeMap<String, RuleSetting>,
    #[serde(default)]
    pub packs: BTreeMap<String, Toggle>,
}

impl Config {
    pub fn parse(file: &Path, text: &str) -> Result<Config, ConfigError> {
        // An empty (or comment-only) file is null in YAML; treat it as the default config.
        if text.lines().all(|l| {
            let t = l.trim();
            t.is_empty() || t.starts_with('#')
        }) {
            return Ok(Config::default());
        }
        serde_yaml_ng::from_str(text).map_err(|e| ConfigError::Parse {
            file: file.to_path_buf(),
            message: e.to_string(),
        })
    }

    pub fn load(file: &Path) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(file).map_err(|source| ConfigError::Io {
            file: file.to_path_buf(),
            source,
        })?;
        Config::parse(file, &text)
    }

    /// Check names against `engine` and switch off packs set to `off`.
    /// Unknown rules/packs and any `core` toggle are errors.
    pub fn apply(&self, file: &Path, engine: &mut Engine) -> Result<(), ConfigError> {
        let invalid = |message: String| ConfigError::Invalid {
            file: file.to_path_buf(),
            message,
        };
        let known: Vec<&str> = engine.rules().map(|r| r.id()).collect();
        for id in self.rules.keys() {
            if !known.contains(&id.as_str()) {
                return Err(invalid(format!(
                    "unknown rule `{id}` (known rules: {})",
                    known.join(", ")
                )));
            }
        }
        for (name, toggle) in &self.packs {
            let Some(pack) = Pack::from_name(name) else {
                return Err(invalid(format!(
                    "unknown pack `{name}` (known packs: aws, gcp, azure)"
                )));
            };
            if pack == Pack::Core {
                return Err(invalid(
                    "the `core` pack cannot be toggled; disable individual rules under `rules:` instead"
                        .to_string(),
                ));
            }
            if *toggle == Toggle::Off {
                engine.disable_pack(pack);
            }
        }
        Ok(())
    }

    /// Drop findings of disabled rules and apply severity overrides.
    pub fn postprocess(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        diagnostics
            .into_iter()
            .filter_map(|mut d| match self.rules.get(&d.rule) {
                Some(RuleSetting::Off) => None,
                Some(RuleSetting::Severity(s)) => {
                    d.severity = *s;
                    Some(d)
                }
                None => Some(d),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(t: &str) -> Result<Config, ConfigError> {
        Config::parse(Path::new(".xplint.yaml"), t)
    }

    #[test]
    fn parses_rules_and_packs() {
        let c = parse(
            "rules:\n  a: off\n  b: error\n  c: warning\npacks:\n  aws: off\n  gcp: on\n  azure: false\n",
        )
        .unwrap();
        assert_eq!(c.rules["a"], RuleSetting::Off);
        assert_eq!(c.rules["b"], RuleSetting::Severity(Severity::Error));
        assert_eq!(c.rules["c"], RuleSetting::Severity(Severity::Warning));
        assert_eq!(c.packs["aws"], Toggle::Off);
        assert_eq!(c.packs["gcp"], Toggle::On);
        assert_eq!(c.packs["azure"], Toggle::Off);
    }

    #[test]
    fn empty_file_is_default() {
        assert_eq!(parse("").unwrap(), Config::default());
        assert_eq!(parse("# nothing\n").unwrap(), Config::default());
    }

    #[test]
    fn rejects_bad_values_and_unknown_keys() {
        assert!(parse("rules:\n  a: loud\n").is_err());
        assert!(parse("packs:\n  aws: maybe\n").is_err());
        assert!(parse("rulez: {}\n").is_err());
    }

    #[test]
    fn apply_rejects_unknown_names_and_core_toggle() {
        let mut e = Engine::with_core();
        let err = parse("rules:\n  nope: off\n")
            .unwrap()
            .apply(Path::new("c"), &mut e)
            .unwrap_err();
        assert!(err.to_string().contains("unknown rule `nope`"));
        let err = parse("packs:\n  oracle: off\n")
            .unwrap()
            .apply(Path::new("c"), &mut e)
            .unwrap_err();
        assert!(err.to_string().contains("unknown pack `oracle`"));
        let err = parse("packs:\n  core: off\n")
            .unwrap()
            .apply(Path::new("c"), &mut e)
            .unwrap_err();
        assert!(err.to_string().contains("core"));
    }

    #[test]
    fn postprocess_disables_and_overrides() {
        let d = |rule: &str, severity| Diagnostic {
            rule: rule.into(),
            severity,
            file: "f".into(),
            line: 1,
            path: String::new(),
            message: String::new(),
        };
        let c = parse("rules:\n  a: off\n  b: error\n").unwrap();
        let out = c.postprocess(vec![
            d("a", Severity::Error),
            d("b", Severity::Warning),
            d("c", Severity::Warning),
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].severity, Severity::Error);
        assert_eq!(out[0].rule, "b");
        assert_eq!(out[1].severity, Severity::Warning);
    }
}
