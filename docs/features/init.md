# Init Feature

The init feature helps repositories create missing sidecars for configured `requireSidecar` paths.

Responsibilities:

- Respect root config exclude and requireSidecar globs.
- Support dry-run output without writing files.
- Generate stable IDs from resource paths.
- Create minimal schema version 1 sidecars.

Implementation:

- `src/init.rs` owns sidecar generation.

Validation:

- Unit tests cover stable ID generation.
- CLI integration tests exercise command behavior through fixture repositories.
