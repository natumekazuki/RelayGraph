# RelayGraph CLI

Use this reference when the task needs command details beyond the quick workflow in `SKILL.md`.

## Validate

Use after editing `.relaygraph.yaml`, sidecars, plugins, or related resources.

```bash
relaygraph validate --json
```

Validation reports graph integrity issues such as missing sidecars, orphan sidecars, duplicate IDs, unresolved locators, unknown kinds, unknown relations, missing required relations, plugin load errors, and schema errors.

## Trace

Use before editing a file, feature root, or design document to discover related design docs, source files, and tests.

```bash
relaygraph trace path:src/main.rs
relaygraph trace id:docs.design.relaygraph
relaygraph trace path:src/main.rs --direction incoming
```

Use `path:` for repository-relative files. Use `id:` for stable resource IDs defined by sidecars.

## Export

Use when another tool needs deterministic graph JSON.

```bash
relaygraph export
```

The default graph JSON output is generated under `._relaygraph/generated/`. Treat it as rebuildable output.

## Cache

Use cache commands for repeated AI-agent queries or external tooling.

```bash
relaygraph cache rebuild
relaygraph cache resources
relaygraph cache resources --json
relaygraph cache links --from path:src/main.rs
relaygraph cache trace path:src/main.rs
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
cargo run -- trace path:src/main.rs
cargo run -- cache rebuild
```
