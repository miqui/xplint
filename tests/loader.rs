use serde_yaml_ng::Value;
use std::path::PathBuf;
use xplint::loader::{load_file, load_str, split_documents, LoadError};
use xplint::model::Resource;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn parses_v1_xrd() {
    let docs = load_file(&fixture("xrd-v1.yaml")).unwrap();
    assert_eq!(docs.len(), 1);
    let Resource::Xrd(x) = &docs[0].resource else {
        panic!("not an XRD")
    };
    assert!(!x.is_v2());
    assert_eq!(x.spec.claim_names.as_ref().unwrap().kind, "Database");
    assert_eq!(x.spec.versions[0].name, "v1alpha1");
    assert!(x.spec.versions[0]
        .schema
        .as_ref()
        .unwrap()
        .open_api_v3_schema
        .is_some());
}

#[test]
fn parses_v2_xrd() {
    let docs = load_file(&fixture("xrd-v2.yaml")).unwrap();
    let Resource::Xrd(x) = &docs[0].resource else {
        panic!("not an XRD")
    };
    assert!(x.is_v2());
    assert_eq!(x.spec.scope.as_deref(), Some("Namespaced"));
    assert!(x.spec.claim_names.is_none());
}

#[test]
fn parses_pipeline_composition() {
    let docs = load_file(&fixture("composition-pipeline.yaml")).unwrap();
    let Resource::Composition(c) = &docs[0].resource else {
        panic!("not a Composition")
    };
    assert!(c.spec.is_pipeline());
    assert_eq!(c.spec.pipeline.len(), 2);
    assert_eq!(
        c.spec.pipeline[0].function_ref.name,
        "function-patch-and-transform"
    );
    assert!(c.spec.pipeline[0].input.is_some());
    assert!(c.spec.pipeline[1].input.is_none());
    assert!(c.metadata.extra.contains_key("labels"));
}

#[test]
fn parses_function() {
    let docs = load_file(&fixture("function.yaml")).unwrap();
    let Resource::Function(f) = &docs[0].resource else {
        panic!("not a Function")
    };
    assert!(f.spec.package.ends_with("v0.8.2"));
}

#[test]
fn multi_doc_keeps_index_and_line_offsets() {
    let docs = load_file(&fixture("multi.yaml")).unwrap();
    let got: Vec<_> = docs
        .iter()
        .map(|d| (d.index, d.start_line, d.resource.kind_name().to_string()))
        .collect();
    assert_eq!(
        got,
        vec![
            (1, 3, "Function".to_string()),
            (2, 10, "ConfigMap".to_string()),
            (3, 17, "CompositeResourceDefinition".to_string()),
        ]
    );
    assert!(matches!(docs[1].resource, Resource::Generic(_)));
}

#[test]
fn malformed_yaml_reports_file_and_document_number() {
    let path = fixture("malformed.yaml");
    let err = load_file(&path).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("malformed.yaml"), "{msg}");
    assert!(msg.contains("document 2"), "{msg}");
    assert!(matches!(err, LoadError::Parse { document: 2, .. }));
}

#[test]
fn typed_kind_with_missing_field_is_error_with_document() {
    let text =
        "---\na: 1\n---\napiVersion: pkg.crossplane.io/v1\nkind: Function\nmetadata: {name: f}\n";
    let err = load_str(std::path::Path::new("f.yaml"), text).unwrap_err();
    assert!(
        matches!(
            err,
            LoadError::Parse {
                document: 2,
                line: 4,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn round_trip_every_fixture() {
    for name in [
        "xrd-v1.yaml",
        "xrd-v2.yaml",
        "composition-pipeline.yaml",
        "function.yaml",
        "multi.yaml",
    ] {
        let text = std::fs::read_to_string(fixture(name)).unwrap();
        let originals: Vec<Value> = split_documents(&text)
            .iter()
            .map(|c| serde_yaml_ng::from_str(c.text).unwrap())
            .collect();
        let docs = load_file(&fixture(name)).unwrap();
        assert_eq!(docs.len(), originals.len(), "{name}");
        for (doc, orig) in docs.iter().zip(&originals) {
            // typed -> value equals original value
            assert_eq!(
                &doc.resource.to_value().unwrap(),
                orig,
                "{name} doc {}",
                doc.index
            );
            // typed -> yaml text -> typed is stable
            let yaml = serde_yaml_ng::to_string(&doc.resource.to_value().unwrap()).unwrap();
            let again = load_str(std::path::Path::new(name), &yaml).unwrap();
            assert_eq!(again[0].resource, doc.resource, "{name} doc {}", doc.index);
        }
    }
}
