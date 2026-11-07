# Test fixtures

## SARIF

- `sarif-schema-2.1.0.json` — the official SARIF 2.1.0 JSON schema (draft-04), vendored unmodified.
  - Source: https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json
    (OASIS SARIF TC, `oasis-tcs/sarif-spec`).
  - License: OASIS TC work product; the repository's `LICENSE.md` points to OASIS policies (GitHub reports spdx
    `NOASSERTION`), with content "freely available for all to use, subject to applicable OASIS policies". Used here only
    as a test fixture.
- `sarif-*.expected.json` — snapshots of `xplint --format sarif` output (run from the repo root over `examples/`). The crate
  version is masked as `@VERSION@`. Regenerate with `UPDATE_SNAPSHOTS=1 cargo test --test sarif` and review the diff.
- `sarif-override.xplint.yaml` — config used to check that severity overrides change a result's `level` but not the rule's
  `defaultConfiguration.level`.

`tests/sarif.rs` validates real CLI output against the schema with the `jsonschema` dev-dependency (full JSON Schema
validation, draft-04). That proves schema conformance only; it does not prove GitHub code scanning accepts the upload.
