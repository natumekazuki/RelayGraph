# Plugin Authoring Guide

Plugins are declarative YAML files. They define resource kinds, relation names, validation rules, and traversal order. Plugins must not execute code.

The default `feature-trace` plugin is bundled into the `relaygraph` binary at `relaygraph/plugins/feature-trace.yaml`, so a fresh repository can validate without a separate plugin file. Custom plugins must be normal repository files under the repo root and referenced by repo-relative paths in `.relaygraph.yaml`; absolute paths and `..` are rejected.

## Minimal Plugin

```yaml
schemaVersion: 1
name: feature-trace
resourceKinds:
  - feature-root
  - source
relations:
  - realized-by
rules:
  - when: feature-root
    requireAnyOutgoing:
      - realized-by
    requireReachableKinds:
      - source
traversal:
  startKinds:
    - feature-root
  relationOrder:
    - realized-by
```

## Fields

- `schemaVersion`: optional, defaults to `1`; only `1` is supported.
- `name`: required plugin name.
- `resourceKinds`: allowed `kind` values in sidecars.
- `relations`: allowed `rel` values in sidecar links.
- `rules[].when`: resource kind the rule applies to.
- `rules[].requireAnyOutgoing`: at least one outgoing relation from this list must exist.
- `rules[].requireReachableKinds`: every listed resource kind must be reachable through outgoing links.
- `traversal.startKinds`: kinds that are natural traversal roots.
- `traversal.relationOrder`: relation priority for deterministic traversal.

## Validation Rules

RelayGraph reports:

- `unknown-kind` when a sidecar or plugin rule references an undeclared kind.
- `unknown-relation` when a link or rule references an undeclared relation.
- `missing-required-relation` when a plugin rule is not satisfied.
- `schema-error` when YAML contains unsupported fields, empty names, or unsupported versions.
- `plugin-load-error` when a configured plugin file cannot be read or parsed.

## Authoring Checklist

1. Add custom plugins under `relaygraph/plugins/`; the default `feature-trace` plugin is already bundled.
2. Add the plugin path to `.relaygraph.yaml`.
3. Declare every kind used by sidecars.
4. Declare every relation used by sidecar links.
5. Add `relationOrder` for stable AI traversal output.
6. Run:

```powershell
cargo run -- validate
cargo run -- export
cargo run -- cache rebuild
```

## When to Create a New Plugin

Create a new plugin when the resource vocabulary or required path differs from `feature-trace`.

Good candidates:

- decision traceability
- asset traceability
- API contract traceability
- test coverage traceability

Keep the Core generic. Put domain language in plugins.
