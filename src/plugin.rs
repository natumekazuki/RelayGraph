use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::diagnostic::validate_schema_version;
use crate::model::{Config, Diagnostic, Plugin, CONFIG_PATH};
use crate::repo::is_reserved_generated_path;
use crate::util::{is_repo_boundary_link, normalize_repo_path, normalize_repo_path_strict};

const DEFAULT_FEATURE_TRACE_PLUGIN: &str = "relaygraph/plugins/feature-trace.yaml";

pub fn load_plugins(
    root: &Path,
    config: &Config,
    discovered_files: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Plugin> {
    let mut plugins = Vec::new();
    let mut plugin_names = BTreeMap::<String, String>::new();
    for path in config.plugins.as_deref().unwrap_or(&[]) {
        let full_path = match resolve_plugin_path(root, path) {
            Ok(full_path) => full_path,
            Err(error) => {
                diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(CONFIG_PATH.to_string()),
                    message: format!("{error:#}"),
                });
                continue;
            }
        };
        if !plugin_is_discovered_or_embedded_default(root, path, discovered_files) {
            diagnostics.push(Diagnostic {
                code: "plugin-load-error",
                path: Some(path.clone()),
                message: "plugin path is not part of Git-backed repository discovery".to_string(),
            });
            continue;
        }
        let repo_path = normalize_plugin_repo_path(path);
        let loaded: Result<Plugin> = read_plugin_text(&full_path, &repo_path)
            .with_context(|| format!("failed to read plugin {path}"))
            .and_then(|text| {
                serde_yaml::from_str::<Plugin>(&text)
                    .with_context(|| format!("failed to parse plugin {path}"))
            });

        match loaded {
            Ok(plugin) => {
                validate_schema_version(plugin.schema_version, path, diagnostics);
                validate_plugin_definition(&plugin, path, diagnostics);
                if let Some(existing) = plugin_names.get(&plugin.name) {
                    diagnostics.push(Diagnostic {
                        code: "duplicate-plugin",
                        path: Some(path.clone()),
                        message: format!(
                            "plugin name {} is already used by {}",
                            plugin.name, existing
                        ),
                    });
                    continue;
                }
                plugin_names.insert(plugin.name.clone(), path.clone());
                plugins.push(plugin);
            }
            Err(error) => diagnostics.push(Diagnostic {
                code: "plugin-load-error",
                path: Some(path.clone()),
                message: format!("{error:#}"),
            }),
        }
    }
    plugins
}

fn plugin_is_discovered_or_embedded_default(
    root: &Path,
    path: &str,
    discovered_files: &BTreeSet<String>,
) -> bool {
    let Ok(repo_path) = validated_plugin_repo_path(path) else {
        return false;
    };
    discovered_files.contains(&repo_path)
        || (repo_path == DEFAULT_FEATURE_TRACE_PLUGIN && !root.join(&repo_path).exists())
}

pub(crate) fn normalize_plugin_repo_path(path: &str) -> String {
    normalize_repo_path(path)
}

pub(crate) fn configured_plugin_paths(config: &Config) -> BTreeSet<String> {
    config
        .plugins
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|path| validated_plugin_repo_path(path).ok())
        .collect()
}

fn read_plugin_text(full_path: &Path, path: &str) -> Result<String> {
    match fs::symlink_metadata(full_path) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!("plugin file must not be a symlink: {path}");
        }
        Ok(_) => {}
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && path == DEFAULT_FEATURE_TRACE_PLUGIN =>
        {
            return Ok(include_str!("../relaygraph/plugins/feature-trace.yaml").to_string());
        }
        Err(error) => return Err(error.into()),
    }

    match fs::read_to_string(full_path) {
        Ok(text) => Ok(text),
        Err(error) => Err(error.into()),
    }
}

fn resolve_plugin_path(root: &Path, path: &str) -> Result<PathBuf> {
    let repo_path = validated_plugin_repo_path(path)?;
    let full_path = root.join(&repo_path);
    let canonical_root = root
        .canonicalize()
        .context("failed to canonicalize repository root")?;
    if let Ok(canonical_plugin) = full_path.canonicalize() {
        if !canonical_plugin.starts_with(&canonical_root) {
            anyhow::bail!("plugin path must stay inside repository: {path}");
        }
    }

    Ok(full_path)
}

fn validated_plugin_repo_path(path: &str) -> Result<String> {
    let repo_path = normalize_repo_path_strict(path).map_err(|_| {
        anyhow::anyhow!("plugin path must be repo-relative and stay inside repository: {path}")
    })?;
    if is_reserved_generated_path(&repo_path) {
        anyhow::bail!("plugin path must not be under reserved generated directory: {path}");
    }
    Ok(repo_path)
}

pub fn build_relation_rank(plugins: &[Plugin]) -> BTreeMap<&str, usize> {
    let mut rank = BTreeMap::new();
    for plugin in plugins {
        let Some(traversal) = &plugin.traversal else {
            continue;
        };
        for relation in &traversal.relation_order {
            if !rank.contains_key(relation.as_str()) {
                let next_rank = rank.len();
                rank.insert(relation.as_str(), next_rank);
            }
        }
    }
    rank
}

fn validate_plugin_definition(plugin: &Plugin, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    if plugin.name.trim().is_empty() {
        diagnostics.push(Diagnostic {
            code: "schema-error",
            path: Some(path.to_string()),
            message: "plugin name must not be empty".to_string(),
        });
    }

    validate_non_empty_unique(
        "resourceKinds",
        &plugin.resource_kinds,
        path,
        "schema-error",
        diagnostics,
    );
    validate_non_empty_unique(
        "relations",
        &plugin.relations,
        path,
        "schema-error",
        diagnostics,
    );

    let kinds = plugin.resource_kinds.iter().collect::<BTreeSet<_>>();
    let relations = plugin.relations.iter().collect::<BTreeSet<_>>();

    for rule in &plugin.rules {
        if rule.when.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: "rule.when must not be empty".to_string(),
            });
        } else if !kinds.contains(&rule.when) {
            diagnostics.push(Diagnostic {
                code: "unknown-kind",
                path: Some(path.to_string()),
                message: format!("rule references unknown kind: {}", rule.when),
            });
        }

        for relation in &rule.require_any_outgoing {
            if relation.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(path.to_string()),
                    message: "rule.requireAnyOutgoing must not contain empty relation".to_string(),
                });
            } else if !relations.contains(relation) {
                diagnostics.push(Diagnostic {
                    code: "unknown-relation",
                    path: Some(path.to_string()),
                    message: format!("rule references unknown relation: {relation}"),
                });
            }
        }

        for kind in &rule.require_reachable_kinds {
            if kind.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(path.to_string()),
                    message: "rule.requireReachableKinds must not contain empty kind".to_string(),
                });
            } else if !kinds.contains(kind) {
                diagnostics.push(Diagnostic {
                    code: "unknown-kind",
                    path: Some(path.to_string()),
                    message: format!("rule references unknown reachable kind: {kind}"),
                });
            }
        }
    }

    if let Some(traversal) = &plugin.traversal {
        for kind in &traversal.start_kinds {
            if kind.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(path.to_string()),
                    message: "traversal.startKinds must not contain empty kind".to_string(),
                });
            } else if !kinds.contains(kind) {
                diagnostics.push(Diagnostic {
                    code: "unknown-kind",
                    path: Some(path.to_string()),
                    message: format!("traversal references unknown kind: {kind}"),
                });
            }
        }

        for relation in &traversal.relation_order {
            if relation.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(path.to_string()),
                    message: "traversal.relationOrder must not contain empty relation".to_string(),
                });
            } else if !relations.contains(relation) {
                diagnostics.push(Diagnostic {
                    code: "unknown-relation",
                    path: Some(path.to_string()),
                    message: format!("traversal references unknown relation: {relation}"),
                });
            }
        }
    }
}

fn validate_non_empty_unique(
    field: &str,
    values: &[String],
    path: &str,
    duplicate_code: &'static str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: format!("{field} must not contain empty value"),
            });
        } else if !seen.insert(value) {
            diagnostics.push(Diagnostic {
                code: duplicate_code,
                path: Some(path.to_string()),
                message: format!("{field} contains duplicate value: {value}"),
            });
        }
    }
}
