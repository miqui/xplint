<img width="500" height="200" alt="image" src="https://github.com/user-attachments/assets/ae50737f-5116-4f60-a5a3-0fe256a4552b" />

# xplint

Opinionated linter for Crossplane (v1 and v2 pipeline-mode) XRDs, Compositions, Functions and rendered output. Written in Rust.

**Status: planning / scaffold.** The binary only prints help and version; no rules exist yet.

## Why
Existing tools cover other layers:

| Tool | Covers | Gap |
|---|---|---|
| [crossplane-contrib/crossplane-lint](https://github.com/crossplane-contrib/crossplane-lint) (Go) | Patch-and-transform era Compositions | Roadmap items unfinished; little pipeline-mode coverage |
| `crossplane beta validate` | Schema validation | No policy or style rules |
| `crossplane render` | Renders output locally | Not a linter |
| xprin | Test framework | Not a linter |

xplint targets opinionated policy rules and pipeline-mode coverage. It is also a vehicle for learning Rust (serde, traits, schema walking, clap, SARIF).

## Planned CLI (not implemented)
```
xplint [PATHS...]                 # lint files or directories
xplint --format json|sarif|text
xplint --config .xplint.yaml
xplint --render                   # lint `crossplane render` output (stage 5)
```
Exit codes: 0 clean, 1 findings, 2 usage/IO error.

## Run manually
Commands run in this repo (Rust 1.98.0):

```
$ cargo test --locked
running 1 test
test tests::greeting_mentions_name ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo run -q -- --help
Opinionated linter for Crossplane XRDs, Compositions and Functions

Usage: xplint

Options:
  -h, --help     Print help
  -V, --version  Print version

$ cargo run -q -- --version
xplint 0.1.0

$ cargo run -q
xplint: scaffold only, no rules implemented yet
```

## Roadmap
See [docs/PLAN.md](docs/PLAN.md). CI design: [docs/CI.md](docs/CI.md).

## License
MIT, see [LICENSE](LICENSE).
