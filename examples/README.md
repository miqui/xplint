# Examples

Small XRD + Composition + Function sets for trying xplint's core rules. The
`clean/` set must produce no findings; each `violations/` set triggers exactly
one rule. `tests/examples.rs` lints this whole directory and asserts the table
below, so it cannot drift.

All files are linted together as one corpus (Functions and XRDs are looked up
across every loaded file), so names are unique per set.

| File | Rule | Severity | Line | Finding |
|---|---|---|---|---|
| `clean/database.yaml` | none | n/a | n/a | XRD with defaults on required fields, Function declared, one base with `deletionPolicy`, one with `managementPolicies`, valid patch paths |
| `violations/required-without-default/xrd.yaml` | `xrd-required-without-default` | warning | 22 | `spec.region` is required but has no default |
| `violations/missing-function/composition.yaml` | `composition-missing-function` | error | 20 | step references `function-not-installed`; only `function-auto-ready` is declared |
| `violations/missing-deletion-policy/composition.yaml` | `composition-missing-deletion-policy` | warning | 22 | base sets neither `deletionPolicy` nor `managementPolicies` |
| `violations/patch-path-not-in-xrd/database.yaml` | `patch-path-not-in-xrd` | error | 56 | `fromFieldPath: spec.sizee` is not in the XRD schema |
| `violations/patch-path-not-in-xrd/database.yaml` | `patch-path-not-in-xrd` | error | 60 | `toFieldPath: status.endpoint` (on the composite) is not in the XRD schema |

Notes:
- `managementPolicies` set explicitly counts as satisfying
  `composition-missing-deletion-policy` (see the rule's docs in the source).
- `patch-path-not-in-xrd` checks only the composite side of a patch; paths on
  the composed resource are not validated.
