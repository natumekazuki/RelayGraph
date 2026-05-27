# Install and Release

RelayGraph v0.1.0 is a Windows x64 preview release. The published artifact is `relaygraph-windows-x64.zip`; Linux and macOS are not official release targets yet.

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
```

## GitHub Release

Use the manual `Release` workflow.

Input:

```text
tag = v0.1.0
```

The workflow builds `relaygraph.exe` on `windows-latest` and uploads:

```text
relaygraph-windows-x64.zip
```

## Versioning Checklist

Before creating a release:

```powershell
# 1. Update Cargo.toml version first.
cargo fmt -- --check
cargo test --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo run --locked -- validate
cargo run --locked -- cache rebuild
cargo run --locked -- cache diagnostics

# 2. Commit the version update, create the matching tag, and push both.
git add Cargo.toml Cargo.lock
git commit -m "chore: release v0.1.0"
git tag v0.1.0
git push origin <default-branch>
git push origin v0.1.0
```

Then run the manual `Release` workflow with the same tag, for example `v0.1.0`.
The workflow verifies that the input tag matches `Cargo.toml` (`version = "0.1.0"` requires tag `v0.1.0`) before building or publishing artifacts.
