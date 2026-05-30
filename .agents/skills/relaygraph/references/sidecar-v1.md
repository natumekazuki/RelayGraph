# Sidecar Schema Version 1

Use sidecars to give repository resources stable IDs, kinds, metadata, and outgoing links.

## Shape

```yaml
schemaVersion: 1
id: docs.feature.example
kind: feature-root
metadata:
  owner: example
links:
  - rel: realized-by
    to: id:src.example
    pathHint: src/example.rs
    order: 10
  - rel: verified-by
    to: id:tests.example
    pathHint: tests/example.rs
    order: 20
```

## Fields

- `schemaVersion`: must be `1`.
- `id`: stable resource ID. Keep it unique in the repository.
- `kind`: resource kind allowed by the configured plugin.
- `metadata`: optional object for repository-specific data.
- `links`: optional ordered outgoing links.
- `pathHint`: optional derived repo-relative target path for an `id:` link.

## Locators

Use only schema version 1 locators:

- `id:<resource-id>`
- `path:<repo-relative-path>`

Prefer `id:` for link targets when the target has a stable sidecar ID. `path:` remains supported in the schema for compatibility and for targets without a useful ID, but CLI link editing commands intentionally accept only ID-based source and target locators.

Do not use absolute paths, parent traversal, or paths outside the repository.

## Path Hints

`to` is the canonical link target. For id-first links, `pathHint` is only a readability hint.

```yaml
links:
  - rel: realized-by
    to: id:src.example
    pathHint: src/example.rs
```

`validate` reports stale or invalid `pathHint` values without writing files. Use `relaygraph link add ... --path-hint` or `relaygraph link update ... --path-hint` to write a hint resolved from a target ID while editing a link. Use `relaygraph sync --dry-run` to preview bulk updates and `relaygraph sync` to refresh existing hints from resolved IDs. `sync` does not add missing hints or migrate all links.

## Bundled Feature Trace Vocabulary

The bundled `feature-trace` plugin defines common resource kinds:

- `feature-root`
- `design-doc`
- `module`
- `source`
- `test`

It also defines common relations:

- `decomposes-to`
- `realized-by`
- `verified-by`

Always check the repository's configured plugin before assuming this vocabulary applies.
