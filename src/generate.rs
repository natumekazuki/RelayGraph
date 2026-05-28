use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{sidecar_suffix, validate_config_values};
use crate::diagnostic::{diagnostics_to_message, validate_schema_version};
use crate::init::{existing_sidecar_ids, unique_generated_id_for_path};
use crate::locator::parse_locator;
use crate::model::{Config, Diagnostic, Locator, Plugin, Sidecar, CONFIG_PATH};
use crate::plugin::{configured_plugin_paths, load_plugins};
use crate::repo::{is_git_ignored, list_repo_files};
use crate::util::{
    display_path, globset, is_repo_boundary_link, matches_glob, normalize_repo_path_strict,
};

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub target: String,
    pub kind: Option<String>,
    pub links: Vec<GenerateLink>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct GenerateLink {
    pub rel: String,
    pub to: String,
}

pub fn parse_generate_link(value: &str) -> std::result::Result<GenerateLink, String> {
    let Some((rel, to)) = value.split_once(':') else {
        return Err("link must use rel:locator syntax".to_string());
    };
    if rel.trim().is_empty() {
        return Err("link relation must not be empty".to_string());
    }
    parse_locator(to)?;
    Ok(GenerateLink {
        rel: rel.to_string(),
        to: to.to_string(),
    })
}

pub fn generate_sidecar(root: &Path, config: &Config, options: GenerateOptions) -> Result<String> {
    let suffix = sidecar_suffix(config);
    let mut diagnostics = Vec::new();
    validate_schema_version(config.schema_version, CONFIG_PATH, &mut diagnostics);
    validate_config_values(config, &mut diagnostics);
    let exclude = globset(config.exclude.as_deref().unwrap_or(&[]), &mut diagnostics);
    if !diagnostics.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&diagnostics));
    }

    let target = normalize_target_path(&options.target)?;
    let files = list_repo_files(root, config.use_git_ignore.unwrap_or(true))?;
    let discovered_files = files.iter().cloned().collect::<BTreeSet<_>>();
    let plugins = load_plugins(root, config, &discovered_files, &mut diagnostics);
    if !diagnostics.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&diagnostics));
    }
    let plugin_paths = configured_plugin_paths(config);
    let link_targets = collect_link_targets(root, &files, &suffix, &exclude, &plugin_paths);
    validate_generate_options(&target, &plugins, &link_targets, &options, &mut diagnostics);
    if !diagnostics.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&diagnostics));
    }

    if target == CONFIG_PATH
        || plugin_paths.contains(&target)
        || target.ends_with(&suffix)
        || matches_glob(&exclude, &target)
    {
        anyhow::bail!("refusing to generate sidecar for non-resource path {target}");
    }
    if !discovered_files.contains(&target) {
        anyhow::bail!("resource path is not part of Git-backed discovery: {target}");
    }

    let resource_path = root.join(&target);
    match fs::symlink_metadata(&resource_path) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!("resource must not be a symlink: {target}");
        }
        Ok(_) => {}
        Err(error) => anyhow::bail!("failed to inspect resource {target}: {error}"),
    }

    let sidecar_path = format!("{target}{suffix}");
    if matches_glob(&exclude, &sidecar_path) {
        anyhow::bail!("generated sidecar would be excluded from discovery: {sidecar_path}");
    }
    if config.use_git_ignore.unwrap_or(true) && is_git_ignored(root, &sidecar_path)? {
        anyhow::bail!("generated sidecar would be ignored by Git discovery: {sidecar_path}");
    }

    match fs::symlink_metadata(root.join(&sidecar_path)) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!("sidecar must not be a symlink: {sidecar_path}");
        }
        Ok(_) => anyhow::bail!("sidecar already exists: {sidecar_path}"),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => anyhow::bail!("failed to inspect sidecar {sidecar_path}: {error}"),
    }

    let mut used_ids = existing_sidecar_ids(root, &files, &suffix);
    let id = unique_generated_id_for_path(&target, &mut used_ids);
    let content = sidecar_content(&id, options.kind.as_deref(), &options.links)?;

    if options.dry_run {
        return Ok(sidecar_path);
    }

    if let Some(parent) = root
        .join(&sidecar_path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", display_path(parent)))?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(&sidecar_path))
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(content.as_bytes())
        })
        .with_context(|| format!("failed to write {sidecar_path}"))?;

    Ok(sidecar_path)
}

fn normalize_target_path(target: &str) -> Result<String> {
    let Some(path) = target.strip_prefix("path:") else {
        anyhow::bail!("generate target must use path: locator");
    };
    normalize_repo_path_strict(path).map_err(|message| anyhow::anyhow!(message))
}

fn validate_generate_options(
    target: &str,
    plugins: &[Plugin],
    link_targets: &LinkTargets,
    options: &GenerateOptions,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(kind) = options.kind.as_deref() {
        if kind.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(target.to_string()),
                message: "kind must not be empty or whitespace".to_string(),
            });
        } else if !plugins.is_empty()
            && !plugins
                .iter()
                .any(|plugin| plugin.resource_kinds.iter().any(|value| value == kind))
        {
            diagnostics.push(Diagnostic {
                code: "unknown-kind",
                path: Some(target.to_string()),
                message: format!("unknown resource kind {kind}"),
            });
        }
    }

    for link in &options.links {
        if !plugins.is_empty()
            && !plugins
                .iter()
                .any(|plugin| plugin.relations.iter().any(|value| value == &link.rel))
        {
            diagnostics.push(Diagnostic {
                code: "unknown-relation",
                path: Some(target.to_string()),
                message: format!("unknown relation {}", link.rel),
            });
        }
        match parse_locator(&link.to) {
            Ok(Locator::Path(path)) => match normalize_repo_path_strict(&path) {
                Ok(path) if link_targets.paths.contains(&path) => {}
                Ok(_) => diagnostics.push(Diagnostic {
                    code: "missing-path",
                    path: Some(target.to_string()),
                    message: format!("unresolved path locator: {}", link.to),
                }),
                Err(message) => diagnostics.push(Diagnostic {
                    code: "schema-error",
                    path: Some(target.to_string()),
                    message: format!("invalid path locator {}: {message}", link.to),
                }),
            },
            Ok(Locator::Id(id)) => match link_targets.ids.get(&id).copied().unwrap_or(0) {
                1 => {}
                0 => diagnostics.push(Diagnostic {
                    code: "unresolved-id",
                    path: Some(target.to_string()),
                    message: format!("unresolved id locator: {}", link.to),
                }),
                _ => diagnostics.push(Diagnostic {
                    code: "ambiguous-id",
                    path: Some(target.to_string()),
                    message: format!("ambiguous id locator: {}", link.to),
                }),
            },
            Err(message) => diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(target.to_string()),
                message: format!("invalid locator {}: {message}", link.to),
            }),
        }
    }
}

struct LinkTargets {
    paths: BTreeSet<String>,
    ids: BTreeMap<String, usize>,
}

fn collect_link_targets(
    root: &Path,
    files: &[String],
    suffix: &str,
    exclude: &globset::GlobSet,
    plugin_paths: &BTreeSet<String>,
) -> LinkTargets {
    let mut paths = BTreeSet::new();
    let mut ids = BTreeMap::new();

    for path in files {
        if path == CONFIG_PATH || plugin_paths.contains(path) || matches_glob(exclude, path) {
            continue;
        }
        if path.ends_with(suffix) {
            if let Ok(text) = fs::read_to_string(root.join(path)) {
                if let Ok(sidecar) = serde_yaml::from_str::<Sidecar>(&text) {
                    if let Some(id) = sidecar.id {
                        *ids.entry(id).or_insert(0) += 1;
                    }
                }
            }
            continue;
        }
        paths.insert(path.clone());
    }

    LinkTargets { paths, ids }
}

fn sidecar_content(id: &str, kind: Option<&str>, links: &[GenerateLink]) -> Result<String> {
    let mut content = format!("schemaVersion: 1\nid: {}\n", quote_yaml_string(id)?);
    if let Some(kind) = kind {
        content.push_str(&format!("kind: {}\n", quote_yaml_string(kind)?));
    }
    if links.is_empty() {
        content.push_str("links: []\n");
    } else {
        content.push_str("links:\n");
        for link in links {
            content.push_str(&format!("  - rel: {}\n", quote_yaml_string(&link.rel)?));
            content.push_str(&format!("    to: {}\n", quote_yaml_string(&link.to)?));
        }
    }
    Ok(content)
}

fn quote_yaml_string(value: &str) -> Result<String> {
    serde_json::to_string(value).context("failed to quote sidecar string")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generate_links() {
        let link = parse_generate_link("verified-by:path:tests/cli.rs").unwrap();
        assert_eq!(link.rel, "verified-by");
        assert_eq!(link.to, "path:tests/cli.rs");
        assert!(parse_generate_link(":path:a.md").is_err());
        assert!(parse_generate_link("verified-by:a.md").is_err());
    }
}
