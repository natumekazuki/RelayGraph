# Export and Trace Feature

Export and trace provide deterministic graph output for humans, AI agents, and external tooling.

Responsibilities:

- Export graph JSON with outgoing and generated incoming links.
- Traverse from `id:` or `path:` locators.
- Support outgoing, incoming, and both-direction traversal.
- Preserve deterministic relation ordering.

Implementation:

- `src/export.rs` owns JSON export shaping.
- `src/trace.rs` owns in-memory traversal.
- `src/cache.rs` owns cache-backed traversal.

Validation:

- `tests/cli.rs` checks cache-backed trace from a fixture repository.
- schema contract tests keep export schema documents parseable and strict.
