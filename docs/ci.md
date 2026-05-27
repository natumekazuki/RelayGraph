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

      - uses: natumekazuki/RelayGraph@v0.1.0
        with:
          export: "true"
          cache: "true"
```

Pin `uses:` to a release tag for reproducible CI. The action installs the CLI
with `cargo install --git` from that tag, so it works on GitHub-hosted Linux,
macOS, and Windows runners without requiring prebuilt release assets for every
platform.

For a monorepo or nested project, set `working-directory`:

```yaml
      - uses: natumekazuki/RelayGraph@v0.1.0
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

The action always runs `relaygraph validate --json`. `export` and `cache` are
optional because they write generated files under the reserved `._relaygraph/`
directory.
