# CLI Feature

The CLI layer owns command parsing and command orchestration.

Responsibilities:

- Parse subcommands and flags.
- Load root configuration once.
- Call graph, export, trace, init, generate, link editing, and cache services.
- Install the bundled RelayGraph Skill into a user-selected skills directory.
- Convert structural and relation freshness diagnostics into process exit codes.
- Keep command handlers thin and side-effect boundaries explicit.

Implementation:

- `src/main.rs` contains only process entry and error reporting.
- `src/cli.rs` contains command definitions and orchestration.
- `src/generate.rs` owns explicit single-sidecar creation.
- `src/link_edit.rs` owns existing sidecar link edits by source and target IDs.
- `src/freshness.rs` owns content fingerprints and relation freshness diagnostics.
- `src/skill.rs` owns bundled Skill installation.

Link editing:

- `link add`, `link remove`, and `link update` select the source resource with `id:<resource-id>`.
- Link arguments use `rel:id:<target-id>`; `path:` link targets are intentionally rejected by the link editing command surface.
- `--path-hint` is a flag that writes or refreshes `pathHint` from the resolved target ID.
- `--reason <text>` sets a non-blank link reason as an explicitly quoted YAML string, and `--clear-reason` removes it. Existing block scalars are replaced as whole fields.
- Setting a reason upgrades the edited sidecar to schema version 3. Existing sibling acknowledgements are migrated with their endpoint revisions preserved and a `linkRevision` added. Removing a reason does not downgrade the sidecar.
- Changing or removing a reason clears relation acknowledgement; setting the same value preserves it.
- `link acknowledge` records the current endpoint revisions and, for version 3, a fingerprint of `rel`, `to`, and `reason`. It upgrades a version 1 sidecar to version 2 while preserving version 3.
- Direct edits to reviewed version 3 link identity or reason are reported as stale by `validate`, even when both endpoint files are unchanged.
- `link acknowledge` can repair a missing version 3 `linkRevision` or remove a version 2 `linkRevision`; unrelated schema errors and malformed revisions remain blocking.
- `validate` reports stale acknowledged relations without failing; `validate --strict` fails when review is required.

Validation:

- `tests/cli.rs` runs the compiled binary against a fixture repository.
