# Graph Build Feature

The graph build layer turns repository files, sidecars, root config, and plugins into a validated in-memory graph.

Responsibilities:

- Discover repository resources.
- Match sidecars to target resources.
- Resolve `id:` and `path:` locators.
- Validate sidecar schema, plugin vocabulary, and plugin rules.
- Preserve deterministic link ordering.

Implementation:

- `src/graph.rs` builds and validates the graph.
- `src/config.rs` loads root config.
- `src/repo.rs` lists repository files.
- `src/plugin.rs` loads plugin contracts.
- `src/model.rs` defines shared data shapes.
- `src/locator.rs` resolves locator syntax.
- `src/diagnostic.rs` formats common diagnostics.
- `src/util.rs` holds path and glob helpers.

Validation:

- Unit tests cover locator parsing, generated IDs, schema errors, reverse trace support, and cache diagnostic preservation.
- CLI integration tests cover graph build through public commands.
