use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{sidecar_suffix, validate_config_values};
use crate::diagnostic::{diagnostics_to_message, validate_schema_version};
use crate::model::{Config, Sidecar, CONFIG_PATH};
use crate::plugin::{configured_plugin_paths, load_plugins};
use crate::repo::{is_git_ignored, list_repo_files};
use crate::util::{display_path, globset, is_repo_boundary_link, matches_glob};

pub fn init_missing_sidecars(root: &Path, config: &Config, dry_run: bool) -> Result<Vec<String>> {
    let suffix = sidecar_suffix(config);
    let mut diagnostics = Vec::new();
    validate_schema_version(config.schema_version, CONFIG_PATH, &mut diagnostics);
    validate_config_values(config, &mut diagnostics);
    let exclude = globset(config.exclude.as_deref().unwrap_or(&[]), &mut diagnostics);
    let require_sidecar = globset(
        config.require_sidecar.as_deref().unwrap_or(&[]),
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&diagnostics));
    }

    let files = list_repo_files(root, config.use_git_ignore.unwrap_or(true))?;
    let discovered_files = files.iter().cloned().collect::<BTreeSet<_>>();
    load_plugins(root, config, &discovered_files, &mut diagnostics);
    if !diagnostics.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&diagnostics));
    }
    let plugin_paths = configured_plugin_paths(config);

    let mut candidates = Vec::new();
    let mut used_ids = existing_sidecar_ids(root, &files, &suffix);

    for path in &files {
        if path == CONFIG_PATH
            || plugin_paths.contains(path)
            || path.ends_with(&suffix)
            || matches_glob(&exclude, path)
            || !matches_glob(&require_sidecar, path)
        {
            continue;
        }

        let resource_path = root.join(path);
        match fs::symlink_metadata(&resource_path) {
            Ok(metadata) if is_repo_boundary_link(&metadata) => {
                diagnostics.push(crate::model::Diagnostic {
                    code: "schema-error",
                    path: Some(path.clone()),
                    message: "resource must not be a symlink".to_string(),
                });
                continue;
            }
            Ok(_) => {}
            Err(error) => {
                diagnostics.push(crate::model::Diagnostic {
                    code: "schema-error",
                    path: Some(path.clone()),
                    message: format!("failed to inspect resource {path}: {error}"),
                });
                continue;
            }
        }

        let sidecar_path = format!("{path}{suffix}");
        if config.use_git_ignore.unwrap_or(true) && is_git_ignored(root, &sidecar_path)? {
            diagnostics.push(crate::model::Diagnostic {
                code: "schema-error",
                path: Some(sidecar_path),
                message: "generated sidecar would be ignored by Git discovery".to_string(),
            });
            continue;
        }

        match fs::symlink_metadata(root.join(&sidecar_path)) {
            Ok(metadata) if is_repo_boundary_link(&metadata) => {
                diagnostics.push(crate::model::Diagnostic {
                    code: "schema-error",
                    path: Some(sidecar_path),
                    message: "sidecar must not be a symlink".to_string(),
                });
                continue;
            }
            Ok(_) => continue,
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                diagnostics.push(crate::model::Diagnostic {
                    code: "schema-error",
                    path: Some(sidecar_path),
                    message: format!("failed to inspect sidecar: {error}"),
                });
                continue;
            }
        }

        let id = unique_generated_id_for_path(path, &mut used_ids);
        candidates.push((sidecar_path, id));
    }

    if !diagnostics.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&diagnostics));
    }

    let created = candidates
        .iter()
        .map(|(sidecar_path, _)| sidecar_path.clone())
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(created);
    }

    for (sidecar_path, id) in candidates {
        if let Some(parent) = root
            .join(&sidecar_path)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", display_path(parent)))?;
        }
        let id = serde_json::to_string(&id).context("failed to quote generated sidecar id")?;
        let content = format!("schemaVersion: 1\nid: {id}\nlinks: []\n");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(&sidecar_path))
            .and_then(|mut file| {
                use std::io::Write;
                file.write_all(content.as_bytes())
            })
            .with_context(|| format!("failed to write {sidecar_path}"))?;
    }

    Ok(created)
}

pub fn generated_id_for_path(path: &str) -> String {
    let mut id = String::new();
    let mut last_was_dot = false;

    for character in path.chars() {
        let next = if character.is_ascii_alphanumeric() {
            last_was_dot = false;
            Some(character.to_ascii_lowercase())
        } else if character == '-' || character == '_' {
            last_was_dot = false;
            Some(character)
        } else if !last_was_dot {
            last_was_dot = true;
            Some('.')
        } else {
            None
        };

        if let Some(character) = next {
            id.push(character);
        }
    }

    let id = id.trim_matches('.').to_string();
    if id.is_empty() {
        format!("resource.{:016x}", stable_path_hash(path))
    } else {
        id
    }
}

fn unique_generated_id_for_path(path: &str, used_ids: &mut BTreeSet<String>) -> String {
    let base = generated_id_for_path(path);
    if used_ids.insert(base.clone()) {
        return base;
    }

    let mut candidate = format!("{base}.{:016x}", stable_path_hash(path));
    let mut counter = 2;
    while !used_ids.insert(candidate.clone()) {
        candidate = format!("{base}.{:016x}.{counter}", stable_path_hash(path));
        counter += 1;
    }
    candidate
}

fn existing_sidecar_ids(root: &Path, files: &[String], suffix: &str) -> BTreeSet<String> {
    files
        .iter()
        .filter(|path| path.ends_with(suffix))
        .filter(|path| {
            fs::symlink_metadata(root.join(path))
                .map(|metadata| !is_repo_boundary_link(&metadata))
                .unwrap_or(false)
        })
        .filter_map(|path| fs::read_to_string(root.join(path)).ok())
        .filter_map(|text| serde_yaml::from_str::<Sidecar>(&text).ok())
        .filter_map(|sidecar| sidecar.id)
        .collect()
}

fn stable_path_hash(path: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_stable_id_from_path() {
        assert_eq!(
            generated_id_for_path("docs/issues/Relay Graph Design.md"),
            "docs.issues.relay.graph.design.md"
        );
        assert_eq!(generated_id_for_path("src/main.rs"), "src.main.rs");
        assert_eq!(generated_id_for_path(".gitignore"), "gitignore");
    }
}
