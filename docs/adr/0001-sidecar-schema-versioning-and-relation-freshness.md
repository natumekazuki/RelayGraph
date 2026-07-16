# ADR 0001: sidecar schema versioning and relation freshness

- Status: Accepted; partially superseded by ADR 0002
- Date: 2026-07-14

## Supersession

ADR 0002 supersedes this ADR where the text limits supported sidecars to versions 1 and 2, requires acknowledgement to be written as version 2, or limits freshness to endpoint fingerprints. Version 3 extends acknowledgement with a reviewed-link fingerprint while adding link reasons. The remaining versioning and file-fingerprint decisions stay in force.

## Context

RelayGraph needs to record when a relation was last reviewed and detect later changes to either endpoint. The existing declaration formats share one version constant, accept only version 1, and reject unknown fields. Adding relation acknowledgement to version 1 would therefore produce version 1 documents that older binaries cannot read safely.

Content fingerprints also need stable behavior across Windows and Unix checkouts, and freshness diagnostics must not make unrelated graph commands fail by default.

## Decision

- Config, plugin, sidecar, and cache format versions are independent from the CLI release version.
- The `Cargo.toml` package version is the canonical CLI release version. Release tags must use `v<package-version>`, and `relaygraph --version` reports that package version.
- A declaration format version changes when a newly written document cannot be interpreted safely by a reader for the previous version. Optional fields count when readers reject unknown fields.
- Missing `schemaVersion` continues to mean sidecar version 1.
- RelayGraph reads sidecar versions 1 and 2. Relation acknowledgement is valid only in version 2.
- `link acknowledge` upgrades only the edited sidecar to version 2. Existing sidecars are not migrated in bulk, and generation without version 2 fields continues to emit version 1.
- A file fingerprint is SHA-256 over LF-normalized UTF-8 content. Non-UTF-8 content is hashed as raw bytes.
- Freshness review diagnostics are produced by `validate`. They do not fail normal validation, but `validate --strict` treats them as failures. Export, trace, and cache behavior is unchanged.
- `sync` never updates acknowledgement. Changing a relation or target clears it; changing `pathHint` or `order` preserves it.

## Alternatives

### Add `acknowledged` to sidecar version 1

Rejected because existing version 1 readers use strict unknown-field rejection and cannot read the new declaration safely.

### Hash raw bytes for every resource

Rejected because equivalent UTF-8 files checked out with CRLF and LF would immediately appear stale across platforms.

### Add warning severity to every graph diagnostic

Rejected for the MVP because it would change export, trace, and cache contracts beyond relation freshness validation.

### Keep the Git tag independent from the Cargo package version

Rejected because the installed CLI could not report the release identity reliably and package metadata could drift from GitHub Release artifacts.

## Consequences

- Sidecar version support is no longer tied to config or plugin version support.
- Tools that author acknowledgement must write version 2 explicitly.
- Line-ending-only changes in UTF-8 files do not require relation review.
- A future incompatible sidecar change advances from version 2; version numbers are not reserved for deferred features.
- The sidecar JSON schema and CLI integration tests are the executable contracts for version and freshness behavior.
- A release requires the Cargo package version, Git tag, CLI version output, and installer metadata to agree.
