# Install and Release

RelayGraph v0.1.0 is a Windows x64 preview release. The published artifact is named `relaygraph-<tag>-windows-x64.zip`; Linux and macOS are validated in CI but do not have official binary artifacts yet.

## Local Install

Prerequisites:

- Rust toolchain with `cargo`
- Git available on `PATH`; `validate`, `export`, `trace`, and `init` use Git-backed discovery by default.

From the repository root:

```powershell
cargo install --path . --locked
relaygraph --help
relaygraph validate
```

Cargo installs the binary under:

```text
%USERPROFILE%\.cargo\bin
```

Make sure that directory is in `PATH`.

## Local Release Build

```powershell
cargo build --locked --release
.\target\release\relaygraph.exe --help
```

The Windows binary is:

```text
target/release/relaygraph.exe
```

## GitHub Actions CI

`.github/workflows/ci.yml` runs:

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo run --locked -- validate
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
tag = v0.1.0
```

The workflow builds `relaygraph.exe` on `windows-latest` and uploads:

```text
relaygraph-<tag>-windows-x64.zip
SHA256SUMS.txt
```

The release version comes from the Git tag. `Cargo.toml` package version is metadata and does not gate the GitHub Release artifact version.

## Versioning Checklist

Before creating a release:

```powershell
# 1. Run local release checks.
cargo fmt -- --check
cargo test --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo run --locked -- validate
cargo run --locked -- cache rebuild
cargo run --locked -- cache diagnostics

# 2. Commit any release notes or metadata updates, then create and push the tag.
git tag v0.1.0
git push origin <default-branch>
git push origin v0.1.0
```

Then run the manual `Release` workflow with the same tag, for example `v0.1.0`.
The workflow verifies tag format and tag checkout integrity before building or publishing artifacts. It does not require the tag to match `Cargo.toml` version.
