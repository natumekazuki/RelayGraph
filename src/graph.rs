use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{sidecar_suffix, validate_config_values};
use crate::diagnostic::validate_schema_version;
use crate::locator::parse_locator;
use crate::model::{
    BuildResult, Config, Diagnostic, Locator, ResolvedLink, Resource, Sidecar, CONFIG_PATH,
};
use crate::plugin::{build_relation_rank, load_plugins};
use crate::repo::list_repo_files_with_diagnostics;
use crate::util::{globset, is_repo_boundary_link, matches_glob, normalize_repo_path};

pub fn build_graph(root: &Path, config: &Config) -> Result<BuildResult> {
    let suffix = sidecar_suffix(config);
    let mut diagnostics = Vec::new();
    validate_schema_version(config.schema_version, CONFIG_PATH, &mut diagnostics);
    validate_config_values(config, &mut diagnostics);

    let files = list_repo_files_with_diagnostics(
        root,
        config.use_git_ignore.unwrap_or(true),
        &mut diagnostics,
    )?;
    let discovered_files = files.iter().cloned().collect::<BTreeSet<_>>();
    let plugins = load_plugins(root, config, &discovered_files, &mut diagnostics);
    let known_kinds = plugins
        .iter()
        .flat_map(|plugin| plugin.resource_kinds.iter().cloned())
        .collect::<BTreeSet<_>>();
    let known_relations = plugins
        .iter()
        .flat_map(|plugin| plugin.relations.iter().cloned())
        .collect::<BTreeSet<_>>();
    let relation_rank = build_relation_rank(&plugins);

    let exclude = globset(config.exclude.as_deref().unwrap_or(&[]), &mut diagnostics);
    let mut resource_paths = BTreeSet::new();
    let mut sidecars = BTreeMap::new();

    for path in files {
        if path == CONFIG_PATH || matches_glob(&exclude, &path) {
            continue;
        }

        let is_boundary_link = fs::symlink_metadata(root.join(&path))
            .map(|metadata| is_repo_boundary_link(&metadata))
            .unwrap_or(false);

        if path.ends_with(&suffix) {
            if is_boundary_link {
                diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(path),
                    message: "sidecar must not be a symlink".to_string(),
                });
                continue;
            }
            if let Some(target) = path.strip_suffix(&suffix) {
                if !target.is_empty() {
                    sidecars.insert(target.to_string(), path);
                }
            }
            continue;
        }

        if is_boundary_link {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path),
                message: "resource must not be a symlink".to_string(),
            });
            continue;
        }

        resource_paths.insert(path);
    }

    collect_sidecar_diagnostics(&resource_paths, &sidecars, config, &mut diagnostics);

    let mut id_to_path = BTreeMap::new();
    let mut duplicate_ids = BTreeSet::new();
    let mut resources = Vec::new();
    let mut loaded_sidecars = BTreeMap::new();

    for path in resource_paths {
        let sidecar_path = sidecars.get(&path).cloned();
        let sidecar = sidecar_path
            .as_deref()
            .and_then(|path| read_sidecar(root, path, &mut diagnostics));

        if let Some(sidecar) = sidecar.as_ref() {
            let sidecar_path = sidecar_path.as_deref().unwrap();
            validate_schema_version(sidecar.schema_version, sidecar_path, &mut diagnostics);
            validate_sidecar_definition(sidecar, sidecar_path, &mut diagnostics);
            if let Some(id) = &sidecar.id {
                if let Some(existing) = id_to_path.get(id) {
                    duplicate_ids.insert(id.clone());
                    diagnostics.push(Diagnostic {
                        code: "duplicate-id",
                        path: Some(sidecar_path.to_string()),
                        message: format!("id {id} is already used by {existing}"),
                    });
                } else {
                    id_to_path.insert(id.clone(), path.clone());
                }
            }
            if let Some(kind) = &sidecar.kind {
                if !known_kinds.is_empty() && !known_kinds.contains(kind) {
                    diagnostics.push(Diagnostic {
                        code: "unknown-kind",
                        path: Some(sidecar_path.to_string()),
                        message: format!("unknown resource kind: {kind}"),
                    });
                }
            }
            loaded_sidecars.insert(sidecar_path.to_string(), sidecar.clone());
        }

        let metadata = sidecar
            .as_ref()
            .filter(|sidecar| metadata_is_json_compatible(&sidecar.metadata))
            .map(|sidecar| sidecar.metadata.clone())
            .unwrap_or_default();

        resources.push(Resource {
            path,
            id: sidecar.as_ref().and_then(|sidecar| sidecar.id.clone()),
            kind: sidecar.as_ref().and_then(|sidecar| sidecar.kind.clone()),
            sidecar: sidecar_path,
            metadata,
            links: Vec::new(),
        });
    }

    let path_set = resources
        .iter()
        .map(|resource| resource.path.clone())
        .collect::<BTreeSet<_>>();

    for resource in &mut resources {
        let Some(sidecar_path) = resource.sidecar.as_deref() else {
            continue;
        };
        let Some(sidecar) = loaded_sidecars.get(sidecar_path) else {
            continue;
        };

        for link in sidecar.links.clone() {
            if !known_relations.is_empty() && !known_relations.contains(&link.rel) {
                diagnostics.push(Diagnostic {
                    code: "unknown-relation",
                    path: Some(sidecar_path.to_string()),
                    message: format!("unknown relation: {}", link.rel),
                });
            }

            let resolved = match parse_locator(&link.to) {
                Ok(Locator::Id(id)) => {
                    let target_path = if duplicate_ids.contains(&id) {
                        diagnostics.push(Diagnostic {
                            code: "ambiguous-id",
                            path: Some(sidecar_path.to_string()),
                            message: format!("ambiguous id locator: {}", link.to),
                        });
                        None
                    } else {
                        id_to_path.get(&id).cloned()
                    };
                    if target_path.is_none() && !duplicate_ids.contains(&id) {
                        diagnostics.push(Diagnostic {
                            code: "unresolved-id",
                            path: Some(sidecar_path.to_string()),
                            message: format!("unresolved id locator: {}", link.to),
                        });
                    }
                    ResolvedLink {
                        rel: link.rel,
                        to: link.to,
                        target_path,
                        target_id: Some(id),
                        order: link.order,
                    }
                }
                Ok(Locator::Path(path)) => {
                    let normalized = normalize_repo_path(path);
                    let target_path = if path_set.contains(&normalized) {
                        Some(normalized.clone())
                    } else {
                        diagnostics.push(Diagnostic {
                            code: "missing-path",
                            path: Some(sidecar_path.to_string()),
                            message: format!("unresolved path locator: {}", link.to),
                        });
                        None
                    };
                    ResolvedLink {
                        rel: link.rel,
                        to: link.to,
                        target_path,
                        target_id: None,
                        order: link.order,
                    }
                }
                Err(message) => {
                    diagnostics.push(Diagnostic {
                        code: "schema-error",
                        path: Some(sidecar_path.to_string()),
                        message,
                    });
                    continue;
                }
            };
            resource.links.push(resolved);
        }
        resource.links.sort_by(|left, right| {
            (
                left.order.unwrap_or(i64::MAX),
                relation_rank
                    .get(left.rel.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                &left.rel,
                &left.to,
            )
                .cmp(&(
                    right.order.unwrap_or(i64::MAX),
                    relation_rank
                        .get(right.rel.as_str())
                        .copied()
                        .unwrap_or(usize::MAX),
                    &right.rel,
                    &right.to,
                ))
        });
    }

    validate_plugin_rules(&resources, &plugins, &mut diagnostics);

    Ok(BuildResult {
        resources,
        diagnostics,
        plugins,
    })
}

fn collect_sidecar_diagnostics(
    resource_paths: &BTreeSet<String>,
    sidecars: &BTreeMap<String, String>,
    config: &Config,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for target in sidecars.keys() {
        if !resource_paths.contains(target) {
            diagnostics.push(Diagnostic {
                code: "orphan-sidecar",
                path: sidecars.get(target).cloned(),
                message: format!("sidecar target does not exist: {target}"),
            });
        }
    }

    let require_sidecar = globset(
        config.require_sidecar.as_deref().unwrap_or(&[]),
        diagnostics,
    );
    for path in resource_paths {
        if matches_glob(&require_sidecar, path) && !sidecars.contains_key(path) {
            diagnostics.push(Diagnostic {
                code: "missing-sidecar",
                path: Some(path.clone()),
                message: "resource matches requireSidecar but has no sidecar".to_string(),
            });
        }
    }
}

fn read_sidecar(root: &Path, path: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<Sidecar> {
    let full_path = root.join(path);
    match fs::symlink_metadata(&full_path) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: "sidecar must not be a symlink".to_string(),
            });
            return None;
        }
        Ok(_) => {}
        Err(error) => {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: format!("failed to inspect sidecar {path}: {error}"),
            });
            return None;
        }
    }
    match fs::read_to_string(&full_path)
        .with_context(|| format!("failed to read sidecar {path}"))
        .and_then(|text| {
            serde_yaml::from_str::<Sidecar>(&text)
                .with_context(|| format!("failed to parse sidecar {path}"))
        }) {
        Ok(sidecar) => Some(sidecar),
        Err(error) => {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: format!("{error:#}"),
            });
            None
        }
    }
}

fn validate_sidecar_definition(sidecar: &Sidecar, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    if sidecar.id.as_deref().is_some_and(|id| id.trim().is_empty()) {
        diagnostics.push(Diagnostic {
            code: "schema-error",
            path: Some(path.to_string()),
            message: "sidecar id must not be empty".to_string(),
        });
    }
    if sidecar
        .kind
        .as_deref()
        .is_some_and(|kind| kind.trim().is_empty())
    {
        diagnostics.push(Diagnostic {
            code: "schema-error",
            path: Some(path.to_string()),
            message: "sidecar kind must not be empty".to_string(),
        });
    }
    for link in &sidecar.links {
        if link.rel.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: "link.rel must not be empty".to_string(),
            });
        }
        if link.to.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(path.to_string()),
                message: "link.to must not be empty".to_string(),
            });
        }
    }
    if !metadata_is_json_compatible(&sidecar.metadata) {
        diagnostics.push(Diagnostic {
            code: "schema-error",
            path: Some(path.to_string()),
            message: "metadata must be JSON-compatible; YAML mapping keys must be strings"
                .to_string(),
        });
    }
}

fn metadata_is_json_compatible(metadata: &BTreeMap<String, serde_yaml::Value>) -> bool {
    serde_json::to_value(metadata).is_ok()
}

fn validate_plugin_rules(
    resources: &[Resource],
    plugins: &[crate::model::Plugin],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for plugin in plugins {
        for rule in &plugin.rules {
            for resource in resources
                .iter()
                .filter(|resource| resource.kind.as_deref() == Some(rule.when.as_str()))
            {
                if !rule.require_any_outgoing.is_empty()
                    && !resource
                        .links
                        .iter()
                        .any(|link| rule.require_any_outgoing.contains(&link.rel))
                {
                    diagnostics.push(Diagnostic {
                        code: "missing-required-relation",
                        path: Some(resource.path.clone()),
                        message: format!(
                            "plugin {} requires one of [{}] for kind {}",
                            plugin.name,
                            rule.require_any_outgoing.join(", "),
                            rule.when
                        ),
                    });
                }
                for missing_kind in
                    missing_reachable_kinds(resources, resource, &rule.require_reachable_kinds)
                {
                    diagnostics.push(Diagnostic {
                        code: "missing-required-relation",
                        path: Some(resource.path.clone()),
                        message: format!(
                            "plugin {} requires reachable resource kind {} for kind {}",
                            plugin.name, missing_kind, rule.when
                        ),
                    });
                }
            }
        }
    }
}

fn missing_reachable_kinds(
    resources: &[Resource],
    start: &Resource,
    required_kinds: &[String],
) -> Vec<String> {
    if required_kinds.is_empty() {
        return Vec::new();
    }

    let by_path = resources
        .iter()
        .map(|resource| (resource.path.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let required = required_kinds.iter().cloned().collect::<BTreeSet<_>>();
    let mut reached = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = start
        .links
        .iter()
        .filter_map(|link| link.target_path.clone())
        .collect::<Vec<_>>();

    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(resource) = by_path.get(path.as_str()) else {
            continue;
        };
        if let Some(kind) = &resource.kind {
            if required.contains(kind) {
                reached.insert(kind.clone());
            }
        }
        pending.extend(
            resource
                .links
                .iter()
                .filter_map(|link| link.target_path.clone()),
        );
    }

    required.difference(&reached).cloned().collect()
}
