//! Rule trait, packs and the engine that runs rules over loaded documents.

mod composition_missing_deletion_policy;
mod composition_missing_function;
mod patch_path_not_in_xrd;
mod xrd_required_without_default;

pub use composition_missing_deletion_policy::CompositionMissingDeletionPolicy;
pub use composition_missing_function::CompositionMissingFunction;
pub use patch_path_not_in_xrd::PatchPathNotInXrd;
pub use xrd_required_without_default::XrdRequiredWithoutDefault;

use crate::diagnostic::{Diagnostic, Severity};
use crate::loader::{Document, SourceDoc};
use crate::model::{split_api_version, Composition, Resource, Xrd};
use crate::yamlpath::YamlPath;
use std::collections::BTreeSet;
use std::path::Path;

/// Which pack a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pack {
    /// Provider-neutral; always on, never disabled by pack toggles.
    Core,
    Aws,
    Gcp,
    Azure,
}

impl Pack {
    /// Name used in config (`packs: { aws: off }`).
    pub fn name(&self) -> &'static str {
        match self {
            Pack::Core => "core",
            Pack::Aws => "aws",
            Pack::Gcp => "gcp",
            Pack::Azure => "azure",
        }
    }

    /// Parse a config pack name (`core`, `aws`, `gcp`, `azure`).
    pub fn from_name(name: &str) -> Option<Pack> {
        [Pack::Core, Pack::Aws, Pack::Gcp, Pack::Azure]
            .into_iter()
            .find(|p| p.name() == name)
    }

    /// Does an API group (version-less) belong to this pack's provider?
    /// Matching ignores the version, so `v1beta1` -> `v1beta2` keeps a pack active.
    pub fn matches_group(&self, group: &str) -> bool {
        let provider = match self {
            Pack::Core => return true,
            Pack::Aws => "aws",
            Pack::Gcp => "gcp",
            Pack::Azure => "azure",
        };
        let root = format!("{provider}.upbound.io");
        let root_m = format!("{provider}.m.upbound.io");
        [root, root_m]
            .iter()
            .any(|r| group == r || group.ends_with(&format!(".{r}")))
    }
}

/// A document under inspection plus the whole loaded corpus for cross-document lookups.
pub struct Doc<'a> {
    pub file: &'a Path,
    pub document: &'a Document,
    pub corpus: &'a [SourceDoc],
}

impl<'a> Doc<'a> {
    /// Build a diagnostic located at `path` in this document.
    pub fn diagnostic(
        &self,
        rule: &str,
        severity: Severity,
        path: &YamlPath,
        message: String,
    ) -> Diagnostic {
        Diagnostic {
            rule: rule.to_string(),
            severity,
            file: self.file.to_path_buf(),
            line: self.document.line_of(path),
            path: path.to_string(),
            message,
        }
    }

    /// All `Function` names (`metadata.name`) in the corpus.
    pub fn function_names(&self) -> Vec<&'a str> {
        self.corpus
            .iter()
            .filter_map(|s| match &s.document.resource {
                Resource::Function(f) => f.metadata.name.as_deref(),
                _ => None,
            })
            .collect()
    }

    /// Find the XRD serving `group/version` + `kind` (compositeTypeRef lookup).
    pub fn find_xrd(&self, api_version: &str, kind: &str) -> Option<&'a Xrd> {
        let (group, version) = split_api_version(api_version);
        self.corpus.iter().find_map(|s| match &s.document.resource {
            Resource::Xrd(x)
                if x.spec.group == group
                    && x.spec.names.kind == kind
                    && x.spec.versions.iter().any(|v| v.name == version) =>
            {
                Some(x)
            }
            _ => None,
        })
    }

    /// The document as a Composition, if it is one.
    pub fn composition(&self) -> Option<&'a Composition> {
        match &self.document.resource {
            Resource::Composition(c) => Some(c),
            _ => None,
        }
    }
}

/// A lint rule.
pub trait Rule {
    fn id(&self) -> &'static str;
    fn severity(&self) -> Severity;
    /// Pack this rule is tagged with; defaults to core.
    fn pack(&self) -> Pack {
        Pack::Core
    }
    /// For pack rules: kinds the rule applies to. Empty means any kind in the pack's groups.
    fn kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn check(&self, doc: &Doc) -> Vec<Diagnostic>;
}

/// Runs registered rules over a corpus.
#[derive(Default)]
pub struct Engine {
    rules: Vec<Box<dyn Rule>>,
    disabled_packs: BTreeSet<Pack>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Engine with the four core rules registered.
    pub fn with_core() -> Self {
        let mut e = Self::new();
        e.register(Box::new(XrdRequiredWithoutDefault));
        e.register(Box::new(CompositionMissingFunction));
        e.register(Box::new(CompositionMissingDeletionPolicy));
        e.register(Box::new(PatchPathNotInXrd));
        e
    }

    pub fn register(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    /// Turn a whole pack off. Core cannot be disabled; the call is ignored.
    pub fn disable_pack(&mut self, pack: Pack) {
        if pack != Pack::Core {
            self.disabled_packs.insert(pack);
        }
    }

    pub fn rules(&self) -> impl Iterator<Item = &dyn Rule> {
        self.rules.iter().map(|r| r.as_ref())
    }

    /// Does `rule` apply to this document? Core: always. Pack: the pack must not
    /// be disabled, and the document's API group (any version) must belong to the
    /// pack and its kind must be in the rule's kind list.
    pub fn applies(&self, rule: &dyn Rule, doc: &Document) -> bool {
        let pack = rule.pack();
        if pack == Pack::Core {
            return true;
        }
        if self.disabled_packs.contains(&pack) {
            return false;
        }
        let (group, _) = split_api_version(doc.resource.api_version());
        if !pack.matches_group(group) {
            return false;
        }
        let kinds = rule.kinds();
        kinds.is_empty() || kinds.contains(&doc.resource.kind_name())
    }

    /// Run every applicable rule on every document; sorted by file, line, rule, path.
    pub fn run(&self, corpus: &[SourceDoc]) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for s in corpus {
            let doc = Doc {
                file: &s.file,
                document: &s.document,
                corpus,
            };
            for rule in &self.rules {
                if self.applies(rule.as_ref(), &s.document) {
                    out.extend(rule.check(&doc));
                }
            }
        }
        out.sort_by(|a, b| {
            (&a.file, a.line, &a.rule, &a.path).cmp(&(&b.file, b.line, &b.rule, &b.path))
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::load_str;
    use std::path::PathBuf;

    struct FakeAws;
    impl Rule for FakeAws {
        fn id(&self) -> &'static str {
            "fake-aws"
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
            vec![doc.diagnostic(self.id(), self.severity(), &YamlPath::new(), "hit".into())]
        }
    }

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
        let mut e = Engine::new();
        e.register(Box::new(FakeAws));
        e
    }

    const RDS: &str = "apiVersion: rds.aws.upbound.io/v1beta1\nkind: Instance\n";

    #[test]
    fn pack_group_matching_ignores_version_and_subgroup() {
        assert!(Pack::Aws.matches_group("rds.aws.upbound.io"));
        assert!(Pack::Aws.matches_group("aws.upbound.io"));
        assert!(Pack::Aws.matches_group("rds.aws.m.upbound.io"));
        assert!(!Pack::Aws.matches_group("rds.gcp.upbound.io"));
        assert!(!Pack::Aws.matches_group("notaws.upbound.io"));
        assert!(Pack::Gcp.matches_group("sql.gcp.upbound.io"));
        assert!(Pack::Azure.matches_group("dbforpostgresql.azure.upbound.io"));
    }

    #[test]
    fn pack_auto_activates_on_group_and_kind() {
        let d = engine().run(&corpus(RDS));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].rule, "fake-aws");
        // a version bump still matches
        let bumped = RDS.replace("v1beta1", "v1beta2");
        assert_eq!(engine().run(&corpus(&bumped)).len(), 1);
    }

    #[test]
    fn pack_does_not_activate_on_other_kind_or_group() {
        let other_kind = "apiVersion: rds.aws.upbound.io/v1beta1\nkind: Cluster\n";
        assert!(engine().run(&corpus(other_kind)).is_empty());
        let other_group = "apiVersion: sql.gcp.upbound.io/v1beta1\nkind: Instance\n";
        assert!(engine().run(&corpus(other_group)).is_empty());
        let xrd = "apiVersion: apiextensions.crossplane.io/v1\nkind: CompositeResourceDefinition\nspec: {group: g, names: {kind: K, plural: ks}}\n";
        assert!(engine().run(&corpus(xrd)).is_empty());
    }

    #[test]
    fn disabled_pack_yields_nothing_but_core_cannot_be_disabled() {
        let mut e = engine();
        e.disable_pack(Pack::Aws);
        assert!(e.run(&corpus(RDS)).is_empty());

        let mut core = Engine::with_core();
        core.disable_pack(Pack::Core);
        assert_eq!(core.rules().count(), 4);
        assert!(core.rules().all(|r| r.pack() == Pack::Core));
    }

    #[test]
    fn diagnostics_are_sorted_by_file_then_line() {
        let mut c = corpus("apiVersion: v1\nkind: A\n---\napiVersion: v1\nkind: B\n");
        c[0].file = PathBuf::from("b.yaml");
        c[1].file = PathBuf::from("a.yaml");
        struct Any;
        impl Rule for Any {
            fn id(&self) -> &'static str {
                "any"
            }
            fn severity(&self) -> Severity {
                Severity::Warning
            }
            fn check(&self, doc: &Doc) -> Vec<Diagnostic> {
                vec![doc.diagnostic(
                    "any",
                    Severity::Warning,
                    &YamlPath::new().key("kind"),
                    "m".into(),
                )]
            }
        }
        let mut e = Engine::new();
        e.register(Box::new(Any));
        let d = e.run(&c);
        assert_eq!(d[0].file, PathBuf::from("a.yaml"));
        assert_eq!(d[1].file, PathBuf::from("b.yaml"));
        assert_eq!(d[0].line, 5); // second doc starts at line 4; `kind` is its second line
        assert_eq!(d[1].line, 2);
    }
}
