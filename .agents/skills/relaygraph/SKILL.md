---
name: relaygraph
description: Use RelayGraph in repositories with .relaygraph.yaml or *.relaygraph.yaml to inspect resource relationships, trace design/source/test impact, validate graph declarations, rebuild or query the cache, and create repository-specific graph rules for AI-assisted code navigation.
---

# RelayGraph

Use RelayGraph to understand and validate Git-backed resource graphs before and after code, documentation, or sidecar changes.

## Core Rules

- Run commands from the repository root that contains `.relaygraph.yaml`.
- Treat `.relaygraph.yaml`, `*.relaygraph.yaml`, and `relaygraph/plugins/*.yaml` as source of truth.
- Treat `._relaygraph/` as generated output. Do not edit it by hand.
- Use repo-relative paths in sidecars. Do not write absolute paths or paths outside the repository.
- Prefer `path:<repo-relative-path>` when tracing a file and `id:<resource-id>` when tracing a stable resource ID.
- Before creating or editing declarations, read repository-local instructions such as `AGENTS.md`, `CLAUDE.md`, and the root `.relaygraph.yaml`.

## Workflow

1. Detect whether the repository uses RelayGraph by checking for `.relaygraph.yaml`.
2. If the repository has no `.relaygraph.yaml`, read `references/bootstrap-repo.md` before creating initial graph files.
3. Read repository-local rules before creating sidecars, running `init`, or changing plugin vocabulary.
4. If root config, plugin vocabulary, or graph granularity is unclear, read the relevant reference before editing.
5. Trace the target file, feature root, or design document before editing.
6. Read the related resources returned by RelayGraph.
7. Make the requested change using existing repository conventions.
8. Validate the graph after changes.

## Command Selection

Use the installed binary when available:

```bash
relaygraph validate --json
relaygraph trace path:src/main.rs --json
relaygraph trace id:docs.design.relaygraph
relaygraph link add id:docs.feature.example realized-by:id:src.example --path-hint
relaygraph link update id:docs.feature.example realized-by:id:src.old --new realized-by:id:src.example --path-hint
relaygraph link remove id:docs.feature.example realized-by:id:src.example
relaygraph export
relaygraph sync --dry-run
relaygraph cache rebuild
relaygraph cache trace path:src/main.rs --json
relaygraph cache diagnostics
```

When working inside the RelayGraph source repository and the binary is not installed, use `cargo run --`:

```bash
cargo run -- validate --json
cargo run -- trace path:src/main.rs --json
cargo run -- link add id:docs.feature.example verified-by:id:tests.example --path-hint
cargo run -- sync --dry-run
cargo run -- cache rebuild
```

For command details, read `references/cli.md`.

## Repository Rules

Repository-local rules override generic examples in this skill. Use them for include/exclude policy, sidecar placement, generated directories, CI-sensitive paths, allowed kinds and relations, ID naming, and validation commands.

When a repository needs new or updated local rules, read `references/repository-rules.md`.

For root config syntax and common config shapes, read `references/config-v1.md`.

For plugin YAML syntax and when to use a repo-local or custom plugin, read `references/plugin-v1.md`.

For initial graph granularity and repository pattern examples, read `references/sample-patterns.md`.

## Sidecars

Sidecars use schema version 1 and repo-relative locators:

```yaml
schemaVersion: 1
id: src.graph
kind: source
links:
  - rel: verified-by
    to: id:tests.cli
    pathHint: tests/cli.rs
```

Use only resource kinds and relations allowed by the configured plugin. For schema examples, read `references/sidecar-v1.md`.

When editing existing sidecar links from the CLI, prefer `relaygraph link add|update|remove` instead of hand-editing YAML. Select the source resource with `id:<resource-id>`, use `rel:id:<target-id>` link arguments, and use `--path-hint` as a flag when the sidecar should store the target path resolved from the target ID.

## Validation

After editing `.relaygraph.yaml`, sidecars, plugins, or linked resources, run:

```bash
relaygraph validate --json
```

If validation reports stale `pathHint` values, run `relaygraph sync --dry-run` first, then `relaygraph sync` when the planned sidecar updates are correct.

If the task changed graph structure, cache behavior, plugins, or many linked resources, also run:

```bash
relaygraph export
relaygraph cache rebuild
relaygraph cache diagnostics
```

Report validation output briefly. If validation cannot be run, state why and describe the remaining graph risk.
