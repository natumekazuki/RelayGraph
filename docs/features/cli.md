# CLI Feature

The CLI layer owns command parsing and command orchestration.

Responsibilities:

- Parse subcommands and flags.
- Load root configuration once.
- Call graph, export, trace, init, and cache services.
- Install the bundled RelayGraph Skill into a user-selected skills directory.
- Convert diagnostics into process exit codes.
- Keep command handlers thin and side-effect boundaries explicit.

Implementation:

- `src/main.rs` contains only process entry and error reporting.
- `src/cli.rs` contains command definitions and orchestration.
- `src/skill.rs` owns bundled Skill installation.

Validation:

- `tests/cli.rs` runs the compiled binary against a fixture repository.
