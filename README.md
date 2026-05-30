# RelayGraph

RelayGraph is a Rust CLI that builds a deterministic resource graph from Git-backed YAML declarations.

v1.0.0 is the initial release, with Windows x64, Linux x64, and macOS arm64 artifacts.

The source of truth is always the repository content:

- `.relaygraph.yaml`
- `*.relaygraph.yaml` sidecars
- YAML plugins under `relaygraph/plugins/`

`._relaygraph/` is a reserved generated directory. It is never discovered as resources or declarations, even if `exclude: []` is configured, and plugins cannot be loaded from there.

The local SQLite database is a generated cache for fast query and AI-agent use. It can always be rebuilt from the Git-backed declarations.

Prerequisites:

- Rust toolchain with `cargo`
- Git available on `PATH`

## Commands

```powershell
cargo run -- validate
cargo run -- validate --json
cargo run -- --help
cargo run -- help generate
cargo run -- init --dry-run
cargo run -- init
cargo run -- generate path:action.yml --kind source --link verified-by:path:tests/cli.rs
cargo run -- generate path:action.yml --dry-run
cargo run -- link add id:docs.root realized-by:id:src.main --path-hint
cargo run -- link update id:docs.root realized-by:id:src.main --new realized-by:id:tests.cli --path-hint
cargo run -- link remove id:docs.root realized-by:id:tests.cli
cargo run -- sync --dry-run
cargo run -- sync
cargo run -- export
cargo run -- trace id:docs.design.relaygraph
cargo run -- trace path:docs/design/relaygraph.md
cargo run -- trace path:src/main.rs --direction incoming
cargo run -- trace path:src/main.rs --json
cargo run -- trace path:src/main.rs --format paths
cargo run -- cache rebuild
cargo run -- cache resources
cargo run -- cache resources --json
cargo run -- cache links --from id:docs.design.relaygraph
cargo run -- cache trace id:docs.design.relaygraph
cargo run -- cache trace path:src/main.rs --direction incoming
cargo run -- cache trace path:src/main.rs --json
cargo run -- cache diagnostics
cargo run -- skill install --to .codex/skills
```

`init` only creates sidecars for paths matched by `requireSidecar`. With the default `requireSidecar: []`, it is expected to be a no-op.

`generate` creates one sidecar for an explicit `path:` resource locator. It refuses excluded, generated, plugin, config, undiscovered, symlinked, ignored, or already-sidecar-backed paths, and it only writes explicitly supplied `kind` and `--link rel:locator` entries.

`link add`, `link remove`, and `link update` edit the `links` list for an existing resource selected by `id:<resource-id>`. Link arguments use `rel:id:<resource-id>` form; updates can replace the relation target with `--new`, set or refresh `pathHint` from the target ID with `--path-hint`, clear `pathHint`, and set or clear `order`. Each command supports `--dry-run`.

New sidecar links should prefer `to: id:<resource-id>` as the canonical target. Optional `pathHint` values are derived readability hints; `validate` reports stale hints without writing, and `sync` refreshes existing hints from resolved IDs.

Default outputs:

- Graph JSON: `._relaygraph/generated/relaygraph.json`
- SQLite cache: `._relaygraph/cache/relaygraph.sqlite`

The entire `._relaygraph/` tree is reserved for generated artifacts and is excluded from discovery independently of the configured `exclude` list.

## Development

```powershell
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## More Docs

- `docs/ci.md`
- `docs/install.md`
- `docs/plugins.md`
- `docs/schema/`

## Initial Scope

- Resources are repository files.
- Sidecars are optional unless matched by `requireSidecar`.
- Locators support `id:` and `path:`.
- `schemaVersion: 1` is supported.
- Plugin relation order is used for deterministic traversal ordering.
- `trace` defaults to `both` direction so generated reverse links are usable from any related resource.
- Default trace output shows relation direction; `--json` is the structured AI/tooling contract and `--format paths` preserves path-only output.
