# RelayGraph CLI

Use this reference when the task needs command details beyond the quick workflow in `SKILL.md`.

## Validate

Use after editing `.relaygraph.yaml`, sidecars, plugins, or related resources.

```bash
relaygraph validate --json
```

Validation reports graph integrity issues such as missing sidecars, orphan sidecars, duplicate IDs, unresolved locators, unknown kinds, unknown relations, missing required relations, plugin load errors, and schema errors.

## Help

Use when checking the installed command surface or a subcommand's arguments.

```bash
relaygraph --help
relaygraph help generate
relaygraph help link add
relaygraph generate --help
```

The top-level help lists available subcommands. Subcommand help shows accepted arguments, flags, and repeatable options.

## Trace

Use before editing a file, feature root, or design document to discover related design docs, source files, and tests.

```bash
relaygraph trace path:src/main.rs
relaygraph trace id:docs.design.relaygraph
relaygraph trace path:src/main.rs --direction incoming
relaygraph trace path:src/main.rs --json
relaygraph trace path:src/main.rs --format paths
```

Use `path:` for repository-relative files. Use `id:` for stable resource IDs defined by sidecars.

Prefer `--json` when another AI agent or tool will consume the result. The JSON output includes the requested start locator, resolved start path, direction, ordered nodes, depth, and each node's incoming or outgoing relation context. Use `--format paths` only when a path-only list is required for compatibility.

## Export

Use when another tool needs deterministic graph JSON.

```bash
relaygraph export
```

The default graph JSON output is generated under `._relaygraph/generated/`. Treat it as rebuildable output.

## Sync

Use after validating sidecars when existing derived readability hints need to be refreshed.

```bash
relaygraph sync --dry-run
relaygraph sync
```

`sync` updates existing `pathHint` values from resolved link targets. It does not add missing hints or migrate all links. `validate` stays read-only and reports stale hints as diagnostics; run `--dry-run` before writing sidecars.

## Link

Use when adding, removing, or updating outgoing links on an existing sidecar-backed resource. Select the source resource by stable ID and use target IDs for link arguments; do not pass sidecar file paths or `path:` link targets.

```bash
relaygraph link add id:docs.feature.example realized-by:id:src.example --path-hint
relaygraph link update id:docs.feature.example realized-by:id:src.old --new realized-by:id:src.example --path-hint
relaygraph link remove id:docs.feature.example realized-by:id:src.example
```

`--path-hint` is a flag. It writes or refreshes `pathHint` from the resolved target ID. `--clear-path-hint` removes an existing hint. Use `--order <N>` or `--clear-order` when traversal order must be explicit.

## Cache

Use cache commands for repeated AI-agent queries or external tooling.

```bash
relaygraph cache rebuild
relaygraph cache resources
relaygraph cache resources --json
relaygraph cache links --from path:src/main.rs
relaygraph cache trace path:src/main.rs
relaygraph cache trace path:src/main.rs --json
relaygraph cache diagnostics
```

Rebuild the cache before relying on cache-backed reads if declarations may have changed.

## Init

Use cautiously. Confirm repository rules first, because `init` may create sidecars.

```bash
relaygraph init --dry-run
relaygraph init
```

Prefer `--dry-run` before writing files. Do not create sidecars in generated, tool-owned, excluded, or CI-sensitive paths unless the repository explicitly allows them.

## Generate

Use when creating one sidecar for an existing Git-backed resource path.

```bash
relaygraph generate path:action.yml --dry-run
relaygraph generate path:action.yml --kind source --link verified-by:path:tests/cli.rs
```

The command writes only explicitly supplied `kind` and `--link rel:locator` values. It rejects excluded resources, excluded sidecar paths, generated paths, plugin/config paths, undiscovered resources, symlink boundaries, Git-ignored sidecars, existing sidecars, unknown vocabulary, and unresolved link targets.

## Skill Install

Use after installing the RelayGraph CLI when the user wants to install or refresh the bundled RelayGraph Skill.

```bash
relaygraph skill install --to .codex/skills
```

The command recreates `<skills-dir>/relaygraph`, so an older saved RelayGraph Skill is removed before the bundled Skill is written again. Do not pass a directory that should be preserved as the Skill itself; pass the parent skills directory.

## Source Repository Fallback

When working inside the RelayGraph source repository and no installed binary is available, use:

```bash
cargo run -- validate --json
cargo run -- help generate
cargo run -- sync --dry-run
cargo run -- trace path:src/main.rs
cargo run -- cache rebuild
```
