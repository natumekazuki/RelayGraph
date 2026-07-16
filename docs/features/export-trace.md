# Export and Trace Feature

Export and trace provide deterministic graph output for humans, AI agents, and external tooling.

Responsibilities:

- Export graph JSON with outgoing and generated incoming links.
- Traverse from `id:` or `path:` locators.
- Support outgoing, incoming, and both-direction traversal.
- Preserve deterministic relation ordering.
- Print direction-aware trace output for humans.
- Provide structured trace JSON for AI agents and external tooling.
- Project optional link reasons through outgoing, incoming, and traversed relations.

Trace output modes:

- Default text output prints the start path, then relation rows such as `from --rel--> to`.
- `--json` prints a stable object with the start resource, requested direction, and ordered nodes.
- `--format paths` preserves path-only output for scripts that only need the reachable file list.

Each structured trace node includes `depth` and, except for the start node, a `via` object with `traversal`, `rel`, `reason`, `from`, and `to`. `from` and `to` always describe the declared relation direction; `traversal` describes whether the trace moved outgoing or incoming from the previously visited node. JSON projections include `reason: null` when no reason is declared. Relation-oriented text output appends a JSON-quoted reason only when one exists.

Implementation:

- `src/export.rs` owns JSON export shaping.
- `src/trace.rs` owns in-memory traversal.
- `src/cache.rs` owns cache-backed traversal.

Validation:

- `tests/cli.rs` checks cache-backed trace from a fixture repository.
- schema contract tests keep export schema documents parseable and strict.
