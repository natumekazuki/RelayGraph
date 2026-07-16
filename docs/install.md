# Install and Release

RelayGraph v1.0.0 is the initial release. Published installers are named `relaygraph-<tag>-<platform>.<extension>`.

## Local Install

Prerequisites:

- Rust toolchain with `cargo`
- Git available on `PATH`; `validate`, `export`, `trace`, and `init` use Git-backed discovery by default.

From the repository root:

```powershell
cargo install --path . --locked
relaygraph --version
relaygraph --help
relaygraph validate
```

Cargo installs the binary under:

```text
%USERPROFILE%\.cargo\bin
```

Make sure that directory is in `PATH`.

## AI Agent Skill

This repository also distributes a RelayGraph Skill for AI agents:

```text
.agents/skills/relaygraph/
```

After installing the RelayGraph CLI, install or refresh the bundled Skill into
your Codex skills directory:

```powershell
$target = if ($env:CODEX_HOME) { Join-Path $env:CODEX_HOME "skills" } else { Join-Path $HOME ".codex\skills" }
relaygraph skill install --to $target
```

The command recreates `$target/relaygraph`, so an older saved RelayGraph Skill is
removed before the bundled Skill is written again.

To install it into Claude Code's personal Skill directory:

```powershell
relaygraph skill install --to (Join-Path $HOME ".claude\skills")
```

For a project-local Claude Code Skill:

```powershell
relaygraph skill install --to .claude/skills
```

Restart the agent session after copying so the Skill metadata is rediscovered.

## Local Release Build

```powershell
cargo build --locked --release
.\target\release\relaygraph.exe --version
.\target\release\relaygraph.exe --help
```

The local Windows binary is:

```text
target/release/relaygraph.exe
```

## GitHub Actions CI

`.github/workflows/ci.yml` runs:

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo run --locked -- validate --strict
cargo run --locked -- cache rebuild
cargo run --locked -- cache diagnostics
cargo build --locked --release
```

CI runs on `windows-latest`, `ubuntu-latest`, and `macos-latest`. It can also be started manually with `workflow_dispatch`.

`.github/workflows/security.yml` runs `cargo audit` on a weekly schedule and manual dispatch. Dependabot checks Cargo and GitHub Actions updates weekly.

## GitHub Release

Use the manual `Release` workflow.

Input:

```text
tag = v1.0.0
```

The workflow builds and uploads installers:

```text
relaygraph-<tag>-windows-x64.msi
relaygraph-<tag>-linux-x64.deb
relaygraph-<tag>-macos-arm64.pkg
SHA256SUMS.txt
```

The Windows MSI installs `relaygraph.exe` under Program Files and appends the
install directory to the system `PATH`. The Linux DEB installs `relaygraph` to
`/usr/bin`. The macOS PKG installs `relaygraph` to `/usr/local/bin`.

The `[package].version` in `Cargo.toml` is the canonical CLI release version. The release tag must be `v<package-version>`, and the release workflow rejects a mismatch before building installers. The same version is reported by `relaygraph --version` and written into platform package metadata.

## Unsigned Installers

Release installers are intentionally unsigned. Windows SmartScreen, macOS
Gatekeeper, or Linux desktop tooling may warn before running the installer.

Every release publishes `SHA256SUMS.txt` so downloaded installers can be checked
against the GitHub Release assets. OS-trusted code signing can be reconsidered
later if the project needs a lower-friction installer experience for a broader
audience.

## Versioning Checklist

Before creating a release:

```powershell
# 1. Set the next SemVer in Cargo.toml and refresh Cargo.lock.
#    Commit both files through the normal pull request workflow.
cargo check

# 2. Confirm that the CLI reports the intended version.
cargo run -- --version

# 3. Run local release checks.
cargo fmt -- --check
cargo test --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo run --locked -- validate --strict
cargo run --locked -- cache rebuild
cargo run --locked -- cache diagnostics

# 4. Merge the version and release notes into the protected default branch.

# 5. Create and push the matching tag from the protected branch tip.
git switch <default-branch>
git pull --ff-only origin <default-branch>
$package = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages |
  Where-Object { $_.name -eq "relaygraph" }
$tag = "v$($package.version)"
git tag $tag
git push origin $tag
```

Then run the manual `Release` workflow with the same tag, for example `v1.3.0`.
The workflow verifies tag format, tag checkout integrity, and equality with the `Cargo.toml` package version before building or publishing artifacts.

When direct pushes to `master` are disabled, release changes go through a normal
pull request first. After the PR is merged, create the release tag locally from
the updated protected branch tip and push only the tag. The manual `Release`
workflow checks out `refs/tags/<tag>`, so the release artifact is built from the
tagged commit, not from the branch used to dispatch the workflow.
