# CLI Feature

The CLI layer owns command parsing and command orchestration.

Responsibilities:

- Parse subcommands and flags.
- Load root configuration once.
- Call graph, export, trace, init, generate, link editing, and cache services.
- Install the bundled RelayGraph Skill into a user-selected skills directory.
- Convert diagnostics into process exit codes.
- Keep command handlers thin and side-effect boundaries explicit.

Implementation:

- `src/main.rs` contains only process entry and error reporting.
- `src/cli.rs` contains command definitions and orchestration.
- `src/generate.rs` owns explicit single-sidecar creation.
- `src/link_edit.rs` owns existing sidecar link edits by source and target IDs.
- `src/skill.rs` owns bundled Skill installation.

Link editing:

- `link add`, `link remove`, and `link update` select the source resource with `id:<resource-id>`.
- Link arguments use `rel:id:<target-id>`; `path:` link targets are intentionally rejected by the link editing command surface.
- `--path-hint` is a flag that writes or refreshes `pathHint` from the resolved target ID.

Validation:

- `tests/cli.rs` runs the compiled binary against a fixture repository.
