# Cache Feature

The cache layer stores a rebuildable SQLite projection of the graph for AI agents and external tools.

Responsibilities:

- Rebuild SQLite cache from Git-backed declarations.
- Store resources, links, plugins, metadata, and diagnostics.
- Reject missing, stale, incomplete, or corrupt cache reads with rebuild guidance.
- Provide cache-backed resource, link, trace, and diagnostic commands.

Implementation:

- `src/cache.rs` owns SQLite schema writes and cache read commands.
- `docs/schema/cache-schema.sql` documents the cache contract.
- Cache reads verify `cacheSchemaVersion`, SQLite `user_version`, integrity, required tables, columns, indexes, and foreign keys.

Validation:

- `tests/cli.rs` checks cache rebuild and cache trace.
- `tests/schema_contract.rs` checks the documented cache schema contract.
