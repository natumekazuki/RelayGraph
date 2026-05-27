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
    to: path:src/example.rs
    order: 10
  - rel: verified-by
    to: path:tests/example.rs
    order: 20
```

## Fields

- `schemaVersion`: must be `1`.
- `id`: stable resource ID. Keep it unique in the repository.
- `kind`: resource kind allowed by the configured plugin.
- `metadata`: optional object for repository-specific data.
- `links`: optional ordered outgoing links.

## Locators

Use only schema version 1 locators:

- `id:<resource-id>`
- `path:<repo-relative-path>`

Do not use absolute paths, parent traversal, or paths outside the repository.

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
