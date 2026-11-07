# Uploading xplint SARIF to GitHub code scanning

Example workflow **snippet** for a repository that uses xplint. It is documentation only; xplint's own
`.github/workflows` are unchanged. It has not been run against a live repository.

```yaml
name: xplint
on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  xplint:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write   # required to upload SARIF
    steps:
      - uses: actions/checkout@v4
      - name: Install xplint
        run: cargo install --git https://github.com/miqui/xplint --locked
      - name: Lint (never fails the step; code scanning reports findings)
        run: xplint --format sarif crossplane/ > xplint.sarif || [ $? -eq 1 ]
      - uses: github/codeql-action/upload-sarif@v3
        if: always()
        with:
          sarif_file: xplint.sarif
          category: xplint
```

Notes:
- Exit code 1 means error-level findings; the `|| [ $? -eq 1 ]` keeps the step green so the upload still runs. Exit 2
  (usage/IO/config error) still fails the step.
- Paths in the SARIF are relative to the directory xplint runs from (the repo root here), as code scanning expects.
- Code scanning on private repositories requires GitHub Advanced Security.
- xplint only reads local YAML files; the job needs no cloud credentials and touches no cloud resources or clusters.
