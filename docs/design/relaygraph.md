# RelayGraph Design

RelayGraph is a Rust CLI for building, validating, exporting, tracing, and caching a generic resource graph from Git-backed declarations.

## Goals

- Treat repository files as generic resources, independent of file format.
- Keep Git-backed YAML declarations as the source of truth.
- Allow resources without sidecars as implicit resources.
- Use optional `*.relaygraph.yaml` sidecars for stable IDs, metadata, and outgoing links.
- Resolve only `id:` and `path:` locators in schema version 1.
- Validate resource kinds, relations, required outgoing links, and required reachable kinds through YAML plugins.
- Generate deterministic traversal and export output.
- Provide a local SQLite cache for AI-agent and tooling use while keeping it fully rebuildable.

## Non-Goals

- A permanent graph database as source of truth.
- Visualization UI.
- Language-specific symbol analysis in schema version 1.
- Executable plugins.
- Fully precise mapping between every possible resource.

## Source of Truth

The source of truth is:

- `.relaygraph.yaml`
- `*.relaygraph.yaml`
- YAML plugins such as `relaygraph/plugins/feature-trace.yaml`

When `useGitIgnore` is true, the root config must also be visible to Git-backed discovery. Ignored `.relaygraph.yaml` files are rejected.

Generated files are not source of truth:

- `._relaygraph/generated/relaygraph.json`
- `._relaygraph/cache/relaygraph.sqlite`

The full `._relaygraph/` tree is a reserved generated area. It is always excluded from repository discovery, even when `exclude: []` is configured, and it cannot contain plugin declarations or source resources.

## Resource Model

Every discovered repository file is a resource unless excluded by config or reserved as generated output. Configured plugin YAML files are declarations, not resources, and are excluded from resource discovery even when `exclude: []` is configured. A resource may have a nearby sidecar:

```text
src/main.rs
src/main.rs.relaygraph.yaml
```

Sidecars are optional unless the path matches `requireSidecar`.

Sidecar schema version 1 supports:

- `schemaVersion`
- `id`
- `kind`
- `metadata`
- `links`

Links are written in one direction. Reverse links are generated for export and trace.

`path:` locators are repository-relative. Current-directory components are folded, while parent traversal and absolute paths are schema errors.

## Deterministic Traversal

Traversal order is deterministic:

1. `order`
2. plugin `traversal.relationOrder`
3. relation name
4. target locator

`trace` defaults to `both` direction so a user can start from either the root document or the implementation resource.

## Plugin Model

Core remains generic. Domain vocabulary belongs in YAML plugins.

Plugins define:

- resource kinds
- relation names
- required outgoing relations
- required reachable resource kinds
- traversal roots and relation order

The bundled plugin is `feature-trace`. It is embedded in the binary for the default path `relaygraph/plugins/feature-trace.yaml`; custom plugins are loaded only from repo-relative paths inside the repository root and Git-backed discovery. Plugin paths under `._relaygraph/` are rejected because that tree is reserved for generated artifacts.

## SQLite Cache

SQLite cache is a required feature but not required for every user workflow.

The cache exists for:

- repeated AI-agent queries
- fast resource and link lookup
- diagnostics inspection
- external tooling integration

The cache is always rebuildable from Git-backed declarations.

Default path:

```text
._relaygraph/cache/relaygraph.sqlite
```

The cache has `metadata.cacheSchemaVersion = 1` and SQLite `PRAGMA user_version = 1`. Read commands verify integrity, required tables, required columns, required indexes, foreign keys, and version metadata. Missing, stale, incomplete, or corrupt caches are rejected with rebuild guidance.

## CLI Surface

```powershell
relaygraph validate
relaygraph validate --json
relaygraph init --dry-run
relaygraph init
relaygraph export
relaygraph trace id:docs.design.relaygraph
relaygraph trace path:src/main.rs
relaygraph cache rebuild
relaygraph cache resources
relaygraph cache links
relaygraph cache trace path:src/main.rs
relaygraph cache diagnostics
```

## Source Layout

`src/main.rs` is intentionally thin. Product behavior lives in focused modules:

- `src/cli.rs`: command parsing and orchestration
- `src/config.rs`: root config loading and defaults
- `src/graph.rs`: resource graph construction and validation
- `src/plugin.rs`: YAML plugin loading and plugin contract validation
- `src/cache.rs`: SQLite cache rebuild and cache-backed queries
- `src/export.rs`: JSON export shape and generated incoming links
- `src/trace.rs`: in-memory traversal
- `src/init.rs`: sidecar generation
- `src/repo.rs`: repository file discovery
- `src/locator.rs`: `id:` and `path:` locator parsing
- `src/diagnostic.rs`: shared diagnostic formatting and version checks
- `src/model.rs`: shared data structures
- `src/util.rs`: path and glob helpers

Feature-level docs live under `docs/features/` and are connected to implementation and test resources through RelayGraph sidecars.

## Error Codes

- `missing-sidecar`
- `orphan-sidecar`
- `duplicate-id`
- `ambiguous-id`
- `unresolved-id`
- `missing-path`
- `unknown-kind`
- `unknown-relation`
- `missing-required-relation`
- `plugin-load-error`
- `duplicate-plugin`
- `schema-error`

## Deferred Work

- schema version 2 locators such as symbol, page, and region
- richer plugin presets
- larger fixture repositories
- performance benchmarks
