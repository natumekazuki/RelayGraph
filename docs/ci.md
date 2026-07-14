# RelayGraph in GitHub Actions

Use the repository action to install RelayGraph from the same tag you pin in
`uses:` and validate another repository's declarations.

```yaml
name: RelayGraph

on:
  pull_request:
  push:
    branches:
      - main
      - master

jobs:
  relaygraph:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: natumekazuki/RelayGraph@v1.0.0
        with:
          strict: "true"
          export: "true"
          cache: "true"
```

Pin `uses:` to a release tag for reproducible CI. The action installs the CLI
with `cargo install --git` from that tag, so it works on GitHub-hosted Linux,
macOS, and Windows runners without requiring prebuilt release assets for every
platform. Set `strict: "true"` to fail when an acknowledged relation requires
review. The default is `"false"` for compatibility with repositories that use
freshness diagnostics as warnings.

For a monorepo or nested project, set `working-directory`:

```yaml
      - uses: natumekazuki/RelayGraph@v1.0.0
        with:
          working-directory: tools/my-project
          export: "true"
          cache: "true"
```

For branch testing before a release, pin both the action and install ref:

```yaml
      - uses: natumekazuki/RelayGraph@master
        with:
          ref: master
```

The action runs `relaygraph validate --json`, adding `--strict` when the strict
input is enabled. `export` and `cache` are optional because they write generated
files under the reserved `._relaygraph/` directory.
