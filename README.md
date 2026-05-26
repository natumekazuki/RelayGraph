# RelayGraph

RelayGraph is a Rust CLI that builds a deterministic resource graph from Git-backed YAML declarations.

The source of truth is always the repository content:

- `.relaygraph.yaml`
- `*.relaygraph.yaml` sidecars
- YAML plugins under `relaygraph/plugins/`

The local SQLite database is a generated cache for fast query and AI-agent use. It can always be rebuilt from the Git-backed declarations.

## Commands

```powershell
cargo run -- validate
cargo run -- validate --json
cargo run -- init --dry-run
cargo run -- init
cargo run -- export
cargo run -- trace id:docs.design.relaygraph
cargo run -- trace path:docs/design/relaygraph.md
cargo run -- trace path:src/main.rs --direction incoming
cargo run -- cache rebuild
cargo run -- cache resources
cargo run -- cache resources --json
cargo run -- cache links --from id:docs.design.relaygraph
cargo run -- cache trace id:docs.design.relaygraph
cargo run -- cache trace path:src/main.rs --direction incoming
cargo run -- cache diagnostics
```

Default outputs:

- Graph JSON: `._relaygraph/generated/relaygraph.json`
- SQLite cache: `._relaygraph/cache/relaygraph.sqlite`

## Development

```powershell
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## More Docs

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
