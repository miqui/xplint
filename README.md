<p align="center">
  <img width="500" height="200" alt="image" src="https://github.com/user-attachments/assets/ae50737f-5116-4f60-a5a3-0fe256a4552b" />
</p>

# xplint

Opinionated linter for Crossplane (v1 and v2 pipeline-mode) XRDs, Compositions, Functions and rendered output. Written in Rust.

**Status: Stage 2 done.** The library parses multi-document YAML into typed models (`xplint::model`, `xplint::loader`) and runs lint rules over them (`xplint::rules::Engine`), producing located diagnostics sorted by file and line. The binary is still a scaffold; the CLI arrives in Stage 3.

## Why
Existing tools cover other layers:

| Tool | Covers | Gap |
|---|---|---|
| [crossplane-contrib/crossplane-lint](https://github.com/crossplane-contrib/crossplane-lint) (Go) | Patch-and-transform era Compositions | Roadmap items unfinished; little pipeline-mode coverage |
| `crossplane beta validate` | Schema validation | No policy or style rules |
| `crossplane render` | Renders output locally | Not a linter |
| xprin | Test framework | Not a linter |

xplint targets opinionated policy rules and pipeline-mode coverage. It is also a vehicle for learning Rust (serde, traits, schema walking, clap, SARIF).

## Flow
![xplint flow](docs/xplint-flow.png)

*Rendered from docs/xplint-flow.drawio — open in draw.io to edit.*

## Rules
Core rules are provider-neutral and always on. Provider packs (`aws`, `gcp`, `azure`) auto-activate per document by API group (any version) plus the rule's kind list; the mechanism is implemented and unit-tested, but no provider rules exist yet (stage 5).

| Rule | Pack | Severity | Checks |
|---|---|---|---|
| `xrd-required-without-default` | core | warning | XRD spec field listed in `required` without a schema `default` |
| `composition-missing-function` | core | error | pipeline step references a function that no loaded `Function` declares (silent if no Function is loaded) |
| `composition-missing-deletion-policy` | core | warning | composed resource `base` sets neither `deletionPolicy` nor `managementPolicies` (an explicit `managementPolicies` counts as satisfying) |
| `patch-path-not-in-xrd` | core | error | composite-side patch `fromFieldPath`/`toFieldPath` not in the XRD OpenAPI schema (skipped if the XRD is not loaded) |

See [examples/](examples/README.md) for sets that trigger each rule.

## Planned CLI (not implemented)
```
xplint [PATHS...]                 # lint files or directories
xplint --format json|sarif|text
xplint --config .xplint.yaml
xplint --render                   # lint `crossplane render` output (stage 5)
```
Exit codes: 0 clean, 1 findings, 2 usage/IO error.

## Run manually
The CLI is Stage 3. Until then, a minimal throwaway harness (a separate crate depending on `xplint` by path, not committed) loaded `examples/` with `xplint::loader::load_tree` and printed `Engine::with_core().run(..)`. Rust 1.98.0:

```
$ cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s

$ cargo test --locked
test result: ok. 22 passed; 0 failed  (unit: model, loader, yamlpath, rules/engine/packs)
test result: ok. 1 passed; 0 failed   (bin)
test result: ok. 1 passed; 0 failed   (tests/examples.rs)
test result: ok. 8 passed; 0 failed   (tests/loader.rs)
test result: ok. 7 passed; 0 failed   (tests/rules.rs)

$ harness examples
examples/violations/missing-deletion-policy/composition.yaml:22: warning [composition-missing-deletion-policy] composed resource 'bucket' sets neither deletionPolicy nor managementPolicies (spec.pipeline[0].input.resources[0].base.spec)
examples/violations/missing-function/composition.yaml:20: error [composition-missing-function] step 'patch-and-transform' references function 'function-not-installed' which is not declared by any loaded Function (spec.pipeline[0].functionRef.name)
examples/violations/patch-path-not-in-xrd/database.yaml:56: error [patch-path-not-in-xrd] path 'spec.sizee' is not defined in the XRD schema (spec.pipeline[0].input.resources[0].patches[0].fromFieldPath)
examples/violations/patch-path-not-in-xrd/database.yaml:60: error [patch-path-not-in-xrd] path 'status.endpoint' is not defined in the XRD schema (spec.pipeline[0].input.resources[0].patches[1].toFieldPath)
examples/violations/required-without-default/xrd.yaml:22: warning [xrd-required-without-default] field 'spec.region' is required but has no default (spec.versions[0].schema.openAPIV3Schema.properties.spec.required[1])

$ harness examples/clean; echo $?
0
```

## Roadmap
See [docs/PLAN.md](docs/PLAN.md). CI design: [docs/CI.md](docs/CI.md).

## License
MIT, see [LICENSE](LICENSE).
