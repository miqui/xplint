# CI design

Written before implementation; `.github/workflows/ci.yml` must match it.

## Triggers
- `pull_request` on every PR (no branch/path filters).
- `push` to `main`.

## Principles
- Keyless: no secrets, no tokens beyond the default read-only `GITHUB_TOKEN` (`permissions: contents: read`).
- Pinned toolchain: Rust `1.98.0`, same version as `rust-toolchain.toml`. Update both together.
- `Cargo.lock` is committed; jobs use `--locked`.
- Actions pinned to major tags: `actions/checkout@v4`, `Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain` (with the explicit toolchain input).

## Jobs
| Job | Blocking | Steps |
|---|---|---|
| `lint` | yes | `cargo fmt --check`; `cargo clippy --all-targets --locked -- -D warnings` |
| `test` | yes | `cargo test --locked` |
| `audit` | no (`continue-on-error: true`) | `cargo install cargo-audit --locked`; `cargo audit` |

Job names are exactly `lint`, `test`, `audit` so they can be used as required status checks (`lint` and `test` only).

## Later
- SARIF upload (`github/codeql-action/upload-sarif`) once stage 4 lands; needs `security-events: write` on that job only.
