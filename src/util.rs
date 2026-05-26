#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::model::{Diagnostic, CONFIG_PATH};

pub fn normalize_repo_path(path: impl AsRef<str>) -> String {
    let path = path.as_ref().replace('\\', "/");
    let mut normalized = PathBuf::new();
    for component in Path::new(&path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized.to_string_lossy().replace('\\', "/")
}

pub fn normalize_repo_path_strict(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    if path.trim().is_empty() {
        return Err("repo path must not be empty".to_string());
    }
    if path.starts_with('/') || path.get(1..2) == Some(":") {
        return Err("repo path must be relative".to_string());
    }

    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err("repo path must not contain parent traversal".to_string()),
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err("repo path must not be empty".to_string());
    }
    Ok(parts.join("/"))
}

pub fn display_path(path: &Path) -> String {
    normalize_repo_path(path.to_string_lossy())
}

pub fn globset(patterns: &[String], diagnostics: &mut Vec<Diagnostic>) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(error) => diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(CONFIG_PATH.to_string()),
                message: format!("invalid glob {pattern}: {error}"),
            }),
        }
    }
    builder
        .build()
        .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap())
}

pub fn matches_glob(globset: &GlobSet, path: &str) -> bool {
    globset.is_match(path)
}

pub fn is_repo_boundary_link(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || is_windows_reparse_point(metadata)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &std::fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}
