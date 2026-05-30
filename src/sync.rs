use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::diagnostic::diagnostics_to_message;
use crate::graph::build_graph;
use crate::locator::parse_locator;
use crate::model::{Config, Locator, Sidecar};
use crate::util::{display_path, normalize_repo_path_strict};

pub fn sync_path_hints(root: &Path, config: &Config, dry_run: bool) -> Result<Vec<String>> {
    let graph = build_graph(root, config)?;
    let blocking = graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code != "path-hint-mismatch")
        .cloned()
        .collect::<Vec<_>>();
    if !blocking.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&blocking));
    }
    let id_to_path = graph
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .id
                .as_deref()
                .map(|id| (id.to_string(), resource.path.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let resource_paths = graph
        .resources
        .iter()
        .map(|resource| resource.path.clone())
        .collect::<BTreeSet<_>>();

    let mut changed = Vec::new();
    let mut planned_writes = Vec::new();
    for resource in graph.resources {
        let Some(sidecar_path) = resource.sidecar else {
            continue;
        };
        let text = read_sidecar_text(root, &sidecar_path)?;
        let sidecar = parse_sidecar(&text, &sidecar_path)?;
        let mut updates = Vec::with_capacity(sidecar.links.len());

        for declared in &sidecar.links {
            let Some(current_hint) = declared.path_hint.as_deref() else {
                updates.push(None);
                continue;
            };
            let Some(target_path) =
                resolved_target_path(&declared.to, &id_to_path, &resource_paths)
            else {
                updates.push(None);
                continue;
            };
            updates.push((current_hint != target_path).then_some(target_path));
        }

        let expected_update_count = updates.iter().filter(|update| update.is_some()).count();
        if expected_update_count > 0 {
            let (updated, applied_update_count) = apply_path_hint_updates(&text, &updates);
            if applied_update_count != expected_update_count {
                anyhow::bail!(
                    "failed to update pathHint values in {sidecar_path}: unsupported links formatting; expected {expected_update_count} update(s), applied {applied_update_count}"
                );
            }
            changed.push(sidecar_path.clone());
            if !dry_run {
                planned_writes.push((sidecar_path.clone(), updated));
            }
        }
    }

    for (sidecar_path, updated) in planned_writes {
        write_sidecar_text(root, &sidecar_path, &updated)?;
    }

    Ok(changed)
}

fn resolved_target_path(
    locator: &str,
    id_to_path: &BTreeMap<String, String>,
    resource_paths: &BTreeSet<String>,
) -> Option<String> {
    match parse_locator(locator).ok()? {
        Locator::Id(id) => id_to_path.get(&id).cloned(),
        Locator::Path(path) => {
            let normalized = normalize_repo_path_strict(&path).ok()?;
            resource_paths.contains(&normalized).then_some(normalized)
        }
    }
}

fn read_sidecar_text(root: &Path, sidecar_path: &str) -> Result<String> {
    fs::read_to_string(root.join(sidecar_path))
        .with_context(|| format!("failed to read sidecar {sidecar_path}"))
}

fn parse_sidecar(text: &str, sidecar_path: &str) -> Result<Sidecar> {
    serde_yaml::from_str(text).with_context(|| format!("failed to parse sidecar {sidecar_path}"))
}

fn write_sidecar_text(root: &Path, sidecar_path: &str, text: &str) -> Result<()> {
    fs::write(root.join(sidecar_path), text)
        .with_context(|| format!("failed to write {}", display_path(&root.join(sidecar_path))))?;
    Ok(())
}

fn apply_path_hint_updates(text: &str, updates: &[Option<String>]) -> (String, usize) {
    let has_trailing_newline = text.ends_with('\n');
    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    let mut in_links = false;
    let mut links_indent = 0;
    let mut link_index = None::<usize>;
    let mut applied = 0usize;

    for line in &mut lines {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if !in_links {
            if indent == 0 && is_links_header(trimmed) {
                in_links = true;
                links_indent = indent;
            }
            continue;
        }

        if indent <= links_indent && !trimmed.is_empty() && !trimmed.starts_with('#') {
            break;
        }
        if indent == links_indent + 2 && trimmed.starts_with("- ") {
            link_index = Some(link_index.map_or(0, |index| index + 1));
            continue;
        }
        if !trimmed.starts_with("pathHint:") {
            continue;
        }
        let Some(target_path) = link_index.and_then(|index| updates.get(index)?.as_ref()) else {
            continue;
        };
        *line = replace_path_hint_value(line, target_path);
        applied += 1;
    }

    let mut updated = lines.join("\n");
    if has_trailing_newline {
        updated.push('\n');
    }
    (updated, applied)
}

fn is_links_header(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("links:") else {
        return false;
    };
    rest.trim().is_empty() || rest.trim_start().starts_with('#')
}

fn replace_path_hint_value(line: &str, target_path: &str) -> String {
    let Some((prefix, value)) = line.split_once(':') else {
        return line.to_string();
    };
    let comment_suffix = yaml_comment_suffix(value);
    let formatted = serde_json::to_string(target_path).unwrap_or_else(|_| target_path.to_string());
    format!("{prefix}: {formatted}{comment_suffix}")
}

fn yaml_comment_suffix(value: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for (index, ch) in value.char_indices() {
        if in_double {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_double = false,
                _ => {}
            }
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '#' if index == 0
                || value[..index]
                    .chars()
                    .last()
                    .is_some_and(|previous| previous.is_whitespace()) =>
            {
                let mut start = index;
                while let Some((previous_index, previous)) =
                    value[..start].char_indices().next_back()
                {
                    if previous.is_whitespace() {
                        start = previous_index;
                    } else {
                        break;
                    }
                }
                return &value[start..];
            }
            _ => {}
        }
    }

    ""
}
