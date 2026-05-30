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
relaygraph export
relaygraph cache rebuild
relaygraph cache trace path:src/main.rs --json
relaygraph cache diagnostics
```

When working inside the RelayGraph source repository and the binary is not installed, use `cargo run --`:

```bash
cargo run -- validate --json
cargo run -- trace path:src/main.rs --json
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
    to: path:tests/cli.rs
```

Use only resource kinds and relations allowed by the configured plugin. For schema examples, read `references/sidecar-v1.md`.

## Validation

After editing `.relaygraph.yaml`, sidecars, plugins, or linked resources, run:

```bash
relaygraph validate --json
```

If the task changed graph structure, cache behavior, plugins, or many linked resources, also run:

```bash
relaygraph export
relaygraph cache rebuild
relaygraph cache diagnostics
```

Report validation output briefly. If validation cannot be run, state why and describe the remaining graph risk.
