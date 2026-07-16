use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::diagnostic::diagnostics_to_message;
use crate::freshness::{acknowledged_revisions, fingerprint_link_revision};
use crate::generate::GenerateLink;
use crate::graph::build_graph;
use crate::locator::parse_locator;
use crate::model::{
    validate_link_reason, AcknowledgedRevisions, Config, Diagnostic, Link, Locator, Sidecar,
    SIDECAR_SCHEMA_VERSION_V1, SIDECAR_SCHEMA_VERSION_V2, SIDECAR_SCHEMA_VERSION_V3,
};
use crate::util::{display_path, is_repo_boundary_link};

#[derive(Debug, Clone)]
pub struct LinkEditOptions {
    pub source: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct AddLinkOptions {
    pub common: LinkEditOptions,
    pub link: GenerateLink,
    pub reason: Option<String>,
    pub path_hint: bool,
    pub order: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RemoveLinkOptions {
    pub common: LinkEditOptions,
    pub link: GenerateLink,
}

#[derive(Debug, Clone)]
pub struct UpdateLinkOptions {
    pub common: LinkEditOptions,
    pub current: GenerateLink,
    pub new_link: Option<GenerateLink>,
    pub path_hint: bool,
    pub clear_path_hint: bool,
    pub reason: Option<String>,
    pub clear_reason: bool,
    pub order: Option<i64>,
    pub clear_order: bool,
}

#[derive(Debug, Clone)]
pub struct AcknowledgeLinkOptions {
    pub common: LinkEditOptions,
    pub link: GenerateLink,
}

pub fn add_link(root: &Path, config: &Config, options: AddLinkOptions) -> Result<String> {
    let context = load_edit_context(root, config, &options.common.source)?;
    let mut link = Link {
        rel: options.link.rel,
        to: options.link.to,
        reason: options.reason,
        path_hint: None,
        order: options.order,
        acknowledged: None,
    };
    let target_path = validate_link(&context, &link)?;
    if options.path_hint {
        link.path_hint = Some(target_path);
    }

    let text = read_sidecar_text(root, &context.sidecar_path)?;
    let mut sidecar = parse_sidecar(&text, &context.sidecar_path)?;
    if sidecar
        .links
        .iter()
        .any(|existing| same_link_target(existing, &link.rel, &link.to))
    {
        anyhow::bail!(
            "link already exists in {}: {}:{}",
            context.sidecar_path,
            link.rel,
            link.to
        );
    }
    let updated = if link.reason.is_some() {
        migrate_sidecar_to_v3(&text, &mut sidecar, None).with_context(|| {
            format!(
                "failed to migrate {} to schemaVersion 3",
                context.sidecar_path
            )
        })?
    } else {
        text
    };
    let updated = apply_link_add(&updated, &link)
        .with_context(|| format!("failed to update links in {}", context.sidecar_path))?;
    write_sidecar_text(
        root,
        &context.sidecar_path,
        &updated,
        options.common.dry_run,
    )?;
    Ok(context.sidecar_path)
}

pub fn remove_link(root: &Path, config: &Config, options: RemoveLinkOptions) -> Result<String> {
    let context = load_edit_context(root, config, &options.common.source)?;
    let text = read_sidecar_text(root, &context.sidecar_path)?;
    let sidecar = parse_sidecar(&text, &context.sidecar_path)?;
    let index = unique_link_index(&sidecar.links, &options.link.rel, &options.link.to)?;
    let updated = apply_link_remove(&text, index)
        .with_context(|| format!("failed to update links in {}", context.sidecar_path))?;
    write_sidecar_text(
        root,
        &context.sidecar_path,
        &updated,
        options.common.dry_run,
    )?;
    Ok(context.sidecar_path)
}

pub fn update_link(root: &Path, config: &Config, options: UpdateLinkOptions) -> Result<String> {
    if options.new_link.is_none()
        && !options.path_hint
        && !options.clear_path_hint
        && options.reason.is_none()
        && !options.clear_reason
        && options.order.is_none()
        && !options.clear_order
    {
        anyhow::bail!(
            "update requires --new, --path-hint, --clear-path-hint, --reason, --clear-reason, --order, or --clear-order"
        );
    }
    if options.path_hint && options.clear_path_hint {
        anyhow::bail!("--path-hint and --clear-path-hint cannot be used together");
    }
    if options.reason.is_some() && options.clear_reason {
        anyhow::bail!("--reason and --clear-reason cannot be used together");
    }
    if options.order.is_some() && options.clear_order {
        anyhow::bail!("--order and --clear-order cannot be used together");
    }

    let context = load_edit_context(root, config, &options.common.source)?;
    let text = read_sidecar_text(root, &context.sidecar_path)?;
    let mut sidecar = parse_sidecar(&text, &context.sidecar_path)?;
    let index = unique_link_index(&sidecar.links, &options.current.rel, &options.current.to)?;
    let original_reason = sidecar.links[index].reason.clone();

    let target_changed = options.new_link.as_ref().is_some_and(|new_link| {
        sidecar.links[index].rel != new_link.rel || sidecar.links[index].to != new_link.to
    });
    if let Some(new_link) = options.new_link {
        sidecar.links[index].rel = new_link.rel;
        sidecar.links[index].to = new_link.to;
    }
    if options.clear_reason {
        sidecar.links[index].reason = None;
    } else if let Some(reason) = options.reason {
        sidecar.links[index].reason = Some(reason);
    }
    let reason_changed = sidecar.links[index].reason != original_reason;
    if target_changed || reason_changed {
        sidecar.links[index].acknowledged = None;
    }
    let target_path = validate_link(&context, &sidecar.links[index])?;
    if options.clear_path_hint {
        sidecar.links[index].path_hint = None;
    } else if options.path_hint || (target_changed && sidecar.links[index].path_hint.is_some()) {
        sidecar.links[index].path_hint = Some(target_path);
    }
    if options.clear_order {
        sidecar.links[index].order = None;
    } else if options.order.is_some() {
        sidecar.links[index].order = options.order;
    }

    if sidecar
        .links
        .iter()
        .enumerate()
        .any(|(other_index, existing)| {
            other_index != index
                && same_link_target(
                    existing,
                    &sidecar.links[index].rel,
                    &sidecar.links[index].to,
                )
        })
    {
        anyhow::bail!(
            "updated link would duplicate existing link in {}: {}:{}",
            context.sidecar_path,
            sidecar.links[index].rel,
            sidecar.links[index].to
        );
    }

    let requires_v3 = sidecar.links[index].reason.is_some();
    let updated = if requires_v3 {
        migrate_sidecar_to_v3(&text, &mut sidecar, Some(index)).with_context(|| {
            format!(
                "failed to migrate {} to schemaVersion 3",
                context.sidecar_path
            )
        })?
    } else {
        text
    };
    let updated = apply_link_update(&updated, index, &sidecar.links[index])
        .with_context(|| format!("failed to update links in {}", context.sidecar_path))?;
    write_sidecar_text(
        root,
        &context.sidecar_path,
        &updated,
        options.common.dry_run,
    )?;
    Ok(context.sidecar_path)
}

pub fn acknowledge_link(
    root: &Path,
    config: &Config,
    options: AcknowledgeLinkOptions,
) -> Result<String> {
    let context =
        load_edit_context_for_acknowledge(root, config, &options.common.source, &options.link)?;
    let text = read_sidecar_text(root, &context.sidecar_path)?;
    let mut sidecar = parse_sidecar(&text, &context.sidecar_path)?;
    let index = unique_link_index(&sidecar.links, &options.link.rel, &options.link.to)?;
    let target_path = validate_link(&context, &sidecar.links[index])?;
    let link_revision = (sidecar.schema_version == Some(SIDECAR_SCHEMA_VERSION_V3)).then(|| {
        fingerprint_link_revision(
            &sidecar.links[index].rel,
            &sidecar.links[index].to,
            sidecar.links[index].reason.as_deref(),
        )
    });
    sidecar.links[index].acknowledged = Some(acknowledged_revisions(
        root,
        &context.source_path,
        &target_path,
        link_revision,
    )?);

    let updated = apply_link_update(&text, index, &sidecar.links[index])
        .with_context(|| format!("failed to update links in {}", context.sidecar_path))?;
    let updated =
        set_min_schema_version(&updated, sidecar.schema_version, SIDECAR_SCHEMA_VERSION_V2);
    write_sidecar_text(
        root,
        &context.sidecar_path,
        &updated,
        options.common.dry_run,
    )?;
    Ok(context.sidecar_path)
}

struct EditContext {
    source_path: String,
    sidecar_path: String,
    known_relations: Vec<String>,
    id_to_path: BTreeMap<String, String>,
}

fn load_edit_context(root: &Path, config: &Config, source: &str) -> Result<EditContext> {
    load_edit_context_with_acknowledgement_repair(root, config, source, None)
}

fn load_edit_context_for_acknowledge(
    root: &Path,
    config: &Config,
    source: &str,
    selected_link: &GenerateLink,
) -> Result<EditContext> {
    load_edit_context_with_acknowledgement_repair(root, config, source, Some(selected_link))
}

fn load_edit_context_with_acknowledgement_repair(
    root: &Path,
    config: &Config,
    source: &str,
    acknowledgement_repair_link: Option<&GenerateLink>,
) -> Result<EditContext> {
    let source_id = match parse_locator(source).map_err(anyhow::Error::msg)? {
        Locator::Id(id) => id,
        Locator::Path(_) => anyhow::bail!("link edit source must use an id: locator"),
    };
    let graph = build_graph(root, config)?;
    let resource = graph
        .resources
        .iter()
        .find(|resource| resource.id.as_deref() == Some(source_id.as_str()))
        .ok_or_else(|| {
            anyhow::anyhow!("source id is not attached to a discovered resource: {source_id}")
        })?;
    let sidecar_path = resource
        .sidecar
        .clone()
        .ok_or_else(|| anyhow::anyhow!("source id has no sidecar: {source_id}"))?;
    let full_path = root.join(&sidecar_path);
    match fs::symlink_metadata(&full_path) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!("sidecar must not be a symlink: {sidecar_path}");
        }
        Ok(_) => {}
        Err(error) => anyhow::bail!("failed to inspect sidecar {sidecar_path}: {error}"),
    }
    let repair_sidecar = if let Some(selected_link) = acknowledgement_repair_link {
        let text = read_sidecar_text(root, &sidecar_path)?;
        let sidecar = parse_sidecar(&text, &sidecar_path)?;
        acknowledgement_shape_repair_is_limited_to_selected_link(&sidecar, selected_link)
            .then_some(sidecar_path.as_str())
    } else {
        None
    };
    let blocking = link_edit_blocking_diagnostics(&graph.diagnostics, repair_sidecar);
    if !blocking.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&blocking));
    }

    let known_relations = graph
        .plugins
        .iter()
        .flat_map(|plugin| plugin.relations.iter().cloned())
        .collect::<Vec<_>>();
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
    Ok(EditContext {
        source_path: resource.path.clone(),
        sidecar_path,
        known_relations,
        id_to_path,
    })
}

fn acknowledgement_shape_repair_is_limited_to_selected_link(
    sidecar: &Sidecar,
    selected_link: &GenerateLink,
) -> bool {
    let schema_version = sidecar.schema_version.unwrap_or(SIDECAR_SCHEMA_VERSION_V1);
    let mut mismatches = sidecar.links.iter().filter(|link| {
        let Some(acknowledged) = &link.acknowledged else {
            return false;
        };
        (schema_version == SIDECAR_SCHEMA_VERSION_V3 && acknowledged.link_revision.is_none())
            || (schema_version == SIDECAR_SCHEMA_VERSION_V2 && acknowledged.link_revision.is_some())
    });
    let Some(mismatch) = mismatches.next() else {
        return false;
    };
    mismatches.next().is_none()
        && mismatch.rel == selected_link.rel
        && mismatch.to == selected_link.to
}

fn read_sidecar_text(root: &Path, sidecar_path: &str) -> Result<String> {
    fs::read_to_string(root.join(sidecar_path))
        .with_context(|| format!("failed to read sidecar {sidecar_path}"))
}

fn parse_sidecar(text: &str, sidecar_path: &str) -> Result<Sidecar> {
    serde_yaml::from_str(text).with_context(|| format!("failed to parse sidecar {sidecar_path}"))
}

fn link_edit_blocking_diagnostics(
    diagnostics: &[Diagnostic],
    acknowledgement_repair_sidecar: Option<&str>,
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            !is_link_edit_repairable_diagnostic(diagnostic.code)
                && !is_acknowledgement_shape_repair_diagnostic(
                    diagnostic,
                    acknowledgement_repair_sidecar,
                )
        })
        .cloned()
        .collect()
}

fn is_acknowledgement_shape_repair_diagnostic(
    diagnostic: &Diagnostic,
    sidecar_path: Option<&str>,
) -> bool {
    diagnostic.code == "schema-error"
        && diagnostic.path.as_deref() == sidecar_path
        && matches!(
            diagnostic.message.as_str(),
            "link.acknowledged.linkRevision is required in sidecar schemaVersion 3"
                | "link.acknowledged.linkRevision requires sidecar schemaVersion 3"
        )
}

fn is_link_edit_repairable_diagnostic(code: &str) -> bool {
    matches!(
        code,
        "path-hint-mismatch"
            | "missing-required-relation"
            | "unknown-relation"
            | "unresolved-id"
            | "missing-path"
            | "missing-sidecar"
    )
}

fn apply_link_add(text: &str, link: &Link) -> Result<String> {
    let mut document = LinkDocument::parse(text)?;
    let new_link = format_link_lines(link, document.link_indent());
    match document.links.as_ref() {
        Some(block) if block.flow_style => {
            anyhow::bail!("unsupported links formatting; use block-style links")
        }
        Some(block) if block.item_ranges.is_empty() => {
            document.lines[block.header_index] = format!("{}links:", " ".repeat(block.indent));
            document
                .lines
                .splice(block.header_index + 1..block.end_index, new_link);
        }
        Some(block) => {
            document
                .lines
                .splice(block.end_index..block.end_index, new_link);
        }
        None => {
            if !document.lines.is_empty() {
                document.lines.push("links:".to_string());
            } else {
                document.lines = vec!["links:".to_string()];
            }
            document.lines.extend(new_link);
        }
    }
    Ok(document.finish())
}

fn apply_link_remove(text: &str, index: usize) -> Result<String> {
    let mut document = LinkDocument::parse(text)?;
    let block = document.block()?;
    if block.flow_style {
        anyhow::bail!("unsupported links formatting; use block-style links");
    }
    let Some(range) = block.item_ranges.get(index).cloned() else {
        anyhow::bail!("link index {index} is not present in links block");
    };
    document.lines.drain(range);
    let remaining = block.item_ranges.len().saturating_sub(1);
    if remaining == 0 {
        let new_block_end = block.end_index.saturating_sub(
            block
                .item_ranges
                .get(index)
                .map_or(0, |range| range.end - range.start),
        );
        document.lines[block.header_index] = format!("{}links: []", " ".repeat(block.indent));
        let start = block.header_index + 1;
        let end = new_block_end;
        if start < end && end <= document.lines.len() {
            document.lines.drain(start..end);
        }
    }
    Ok(document.finish())
}

fn apply_link_update(text: &str, index: usize, link: &Link) -> Result<String> {
    let mut document = LinkDocument::parse(text)?;
    let block = document.block()?;
    if block.flow_style {
        anyhow::bail!("unsupported links formatting; use block-style links");
    }
    let Some(range) = block.item_ranges.get(index).cloned() else {
        anyhow::bail!("link index {index} is not present in links block");
    };
    update_link_range(&mut document.lines, range, link, block.item_indent())?;
    Ok(document.finish())
}

fn apply_link_revision_migration(
    text: &str,
    index: usize,
    acknowledged: &AcknowledgedRevisions,
) -> Result<String> {
    let mut document = LinkDocument::parse(text)?;
    let block = document.block()?;
    if block.flow_style {
        anyhow::bail!("unsupported links formatting; use block-style links");
    }
    let Some(range) = block.item_ranges.get(index).cloned() else {
        anyhow::bail!("link index {index} is not present in links block");
    };
    let item_indent = block.item_indent();
    let field_indent = item_indent + 2;
    let Some(start) = range.clone().find(|line_index| {
        link_field_line(&document.lines[*line_index], item_indent, "acknowledged")
    }) else {
        anyhow::bail!("link index {index} has no acknowledgement to migrate");
    };
    let (prefix, current_value) = document.lines[start]
        .split_once(':')
        .expect("acknowledgement field contains a colon");
    let prefix = prefix.to_string();
    let header_comment = yaml_comment_suffix(current_value).to_string();
    let inline_value = current_value[..current_value.len() - header_comment.len()].trim();
    if !inline_value.is_empty() {
        document.lines[start] = format!("{prefix}:{header_comment}");
        document.lines.splice(
            start + 1..start + 1,
            [
                format!(
                    "{}sourceRevision: {}",
                    " ".repeat(field_indent + 2),
                    acknowledged.source_revision
                ),
                format!(
                    "{}targetRevision: {}",
                    " ".repeat(field_indent + 2),
                    acknowledged.target_revision
                ),
                format!(
                    "{}linkRevision: {}",
                    " ".repeat(field_indent + 2),
                    acknowledged
                        .link_revision
                        .as_deref()
                        .expect("v3 migration provides a link revision")
                ),
            ],
        );
        return Ok(document.finish());
    }
    let end = document
        .lines
        .iter()
        .enumerate()
        .take(range.end)
        .skip(start + 1)
        .find_map(|(line_index, line)| {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            (indent <= field_indent && !trimmed.is_empty() && !trimmed.starts_with('#'))
                .then_some(line_index)
        })
        .unwrap_or(range.end);
    document.lines.insert(
        end,
        format!(
            "{}linkRevision: {}",
            " ".repeat(field_indent + 2),
            acknowledged
                .link_revision
                .as_deref()
                .expect("v3 migration provides a link revision")
        ),
    );
    Ok(document.finish())
}

#[derive(Clone)]
struct LinkBlock {
    header_index: usize,
    end_index: usize,
    indent: usize,
    flow_style: bool,
    item_ranges: Vec<std::ops::Range<usize>>,
}

impl LinkBlock {
    fn item_indent(&self) -> usize {
        self.indent + 2
    }
}

struct LinkDocument {
    lines: Vec<String>,
    trailing_newline: bool,
    links: Option<LinkBlock>,
}

impl LinkDocument {
    fn parse(text: &str) -> Result<Self> {
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
        let links = find_links_block(&lines)?;
        Ok(Self {
            lines,
            trailing_newline,
            links,
        })
    }

    fn block(&self) -> Result<LinkBlock> {
        self.links
            .clone()
            .ok_or_else(|| anyhow::anyhow!("sidecar has no links field"))
    }

    fn link_indent(&self) -> usize {
        self.links.as_ref().map_or(2, LinkBlock::item_indent)
    }

    fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        if self.trailing_newline || !text.is_empty() {
            text.push('\n');
        }
        text
    }
}

fn find_links_block(lines: &[String]) -> Result<Option<LinkBlock>> {
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == 0 && is_links_header(trimmed) {
            let flow_style = !is_plain_links_header(trimmed) && !is_empty_links_header(trimmed);
            let end_index = find_block_end(lines, index + 1, indent);
            let item_ranges = if flow_style {
                Vec::new()
            } else {
                link_item_ranges(lines, index + 1, end_index, indent + 2)
            };
            return Ok(Some(LinkBlock {
                header_index: index,
                end_index,
                indent,
                flow_style,
                item_ranges,
            }));
        }
    }
    Ok(None)
}

fn find_block_end(lines: &[String], start: usize, header_indent: usize) -> usize {
    for (offset, line) in lines[start..].iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent <= header_indent && !trimmed.is_empty() && !trimmed.starts_with('#') {
            return start + offset;
        }
    }
    lines.len()
}

fn link_item_ranges(
    lines: &[String],
    start: usize,
    end: usize,
    item_indent: usize,
) -> Vec<std::ops::Range<usize>> {
    let mut starts = Vec::new();
    for (offset, line) in lines[start..end].iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == item_indent && is_sequence_item_start(trimmed) {
            starts.push(start + offset);
        }
    }
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let range_end = starts.get(index + 1).copied().unwrap_or(end);
            *start..range_end
        })
        .collect()
}

fn update_link_range(
    lines: &mut Vec<String>,
    range: std::ops::Range<usize>,
    link: &Link,
    item_indent: usize,
) -> Result<()> {
    let link_start = range.start;
    replace_required_link_field(lines, range.clone(), item_indent, "rel", &link.rel)?;
    replace_required_link_field(lines, range.clone(), item_indent, "to", &link.to)?;
    update_optional_string_field(
        lines,
        range.clone(),
        item_indent,
        "pathHint",
        &link.path_hint,
    );
    let range = link_range_from_start(lines, link_start, item_indent);
    update_optional_reason_field(lines, range, item_indent, &link.reason);
    let range = link_range_from_start(lines, link_start, item_indent);
    update_optional_i64_field(lines, range, item_indent, "order", link.order);
    let range = link_range_from_start(lines, link_start, item_indent);
    update_acknowledged_field(lines, range, item_indent, link.acknowledged.as_ref());
    Ok(())
}

fn update_acknowledged_field(
    lines: &mut Vec<String>,
    range: std::ops::Range<usize>,
    item_indent: usize,
    acknowledged: Option<&AcknowledgedRevisions>,
) {
    let field_indent = item_indent + 2;
    let field_index = range
        .clone()
        .find(|index| link_field_line(&lines[*index], item_indent, "acknowledged"));
    let mut sequence_marker_field = false;
    let mut header_comment = String::new();
    if let Some(start) = field_index {
        let trimmed = lines[start].trim_start();
        sequence_marker_field =
            lines[start].len() - trimmed.len() == item_indent && trimmed.starts_with("- ");
        if let Some((_, value)) = lines[start].split_once(':') {
            header_comment = yaml_comment_suffix(value).to_string();
        }
        let end = lines
            .iter()
            .enumerate()
            .take(range.end)
            .skip(start + 1)
            .find_map(|(index, line)| {
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                (indent <= field_indent && !trimmed.is_empty() && !trimmed.starts_with('#'))
                    .then_some(index)
            })
            .unwrap_or(range.end);
        lines.drain(start..end);
        if acknowledged.is_none() && sequence_marker_field {
            lines.insert(start, format!("{}-", " ".repeat(item_indent)));
        }
    }
    let Some(acknowledged) = acknowledged else {
        return;
    };
    let range = link_range_from_start(lines, range.start, item_indent);
    let nested_indent = field_indent + 2;
    let acknowledged_line = if sequence_marker_field {
        format!(
            "{}- acknowledged:{}",
            " ".repeat(item_indent),
            header_comment
        )
    } else {
        format!(
            "{}acknowledged:{}",
            " ".repeat(field_indent),
            header_comment
        )
    };
    let insert_at = if sequence_marker_field {
        range.start
    } else {
        range.end
    };
    lines.splice(
        insert_at..insert_at,
        [
            acknowledged_line,
            format!(
                "{}sourceRevision: {}",
                " ".repeat(nested_indent),
                acknowledged.source_revision
            ),
            format!(
                "{}targetRevision: {}",
                " ".repeat(nested_indent),
                acknowledged.target_revision
            ),
        ],
    );
    if let Some(link_revision) = &acknowledged.link_revision {
        let range = link_range_from_start(lines, range.start, item_indent);
        lines.insert(
            range.end,
            format!(
                "{}linkRevision: {}",
                " ".repeat(nested_indent),
                link_revision
            ),
        );
    }
}

fn set_schema_version(text: &str, version: u32) -> String {
    let trailing_newline = text.ends_with('\n');
    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    if let Some(index) = lines.iter().position(|line| {
        let trimmed = line.trim_start();
        line.len() == trimmed.len() && field_line(trimmed, "schemaVersion")
    }) {
        lines[index] = replace_yaml_value(&lines[index], &version.to_string());
    } else {
        lines.insert(0, format!("schemaVersion: {version}"));
    }
    let mut updated = lines.join("\n");
    if trailing_newline || !updated.is_empty() {
        updated.push('\n');
    }
    updated
}

fn set_min_schema_version(text: &str, current: Option<u32>, minimum: u32) -> String {
    let current = current.unwrap_or(SIDECAR_SCHEMA_VERSION_V1);
    if current >= minimum {
        text.to_string()
    } else {
        set_schema_version(text, minimum)
    }
}

fn migrate_sidecar_to_v3(
    text: &str,
    sidecar: &mut Sidecar,
    deferred_link_index: Option<usize>,
) -> Result<String> {
    if sidecar.schema_version.unwrap_or(SIDECAR_SCHEMA_VERSION_V1) >= SIDECAR_SCHEMA_VERSION_V3 {
        return Ok(text.to_string());
    }

    let mut migrated_indices = Vec::new();
    for (index, link) in sidecar.links.iter_mut().enumerate() {
        if link
            .acknowledged
            .as_ref()
            .is_some_and(|acknowledged| acknowledged.link_revision.is_none())
        {
            let link_revision =
                fingerprint_link_revision(&link.rel, &link.to, link.reason.as_deref());
            link.acknowledged
                .as_mut()
                .expect("acknowledgement presence was checked")
                .link_revision = Some(link_revision.clone());
            migrated_indices.push(index);
        }
    }

    let mut updated = text.to_string();
    for index in migrated_indices {
        if Some(index) != deferred_link_index {
            updated = apply_link_revision_migration(
                &updated,
                index,
                sidecar.links[index]
                    .acknowledged
                    .as_ref()
                    .expect("migrated link remains acknowledged"),
            )?;
        }
    }
    sidecar.schema_version = Some(SIDECAR_SCHEMA_VERSION_V3);
    Ok(set_schema_version(&updated, SIDECAR_SCHEMA_VERSION_V3))
}

fn replace_required_link_field(
    lines: &mut [String],
    range: std::ops::Range<usize>,
    item_indent: usize,
    field: &str,
    value: &str,
) -> Result<()> {
    for index in range {
        if link_field_line(&lines[index], item_indent, field) {
            lines[index] = replace_yaml_value(&lines[index], &format_yaml_string(value));
            return Ok(());
        }
    }
    anyhow::bail!("unsupported links formatting; missing {field} field")
}

fn update_optional_string_field(
    lines: &mut Vec<String>,
    range: std::ops::Range<usize>,
    item_indent: usize,
    field: &str,
    value: &Option<String>,
) {
    update_optional_field(
        lines,
        range,
        item_indent,
        field,
        value.as_deref().map(format_yaml_string),
    );
}

fn update_optional_reason_field(
    lines: &mut Vec<String>,
    range: std::ops::Range<usize>,
    item_indent: usize,
    value: &Option<String>,
) {
    let field_index = range
        .clone()
        .find(|index| link_field_line(&lines[*index], item_indent, "reason"));
    match (field_index, value) {
        (Some(start), Some(value)) => {
            let end = link_field_end(lines, start, range.end, item_indent);
            let replacement = replace_yaml_value(&lines[start], &format_yaml_quoted_string(value));
            lines.splice(start..end, [replacement]);
        }
        (Some(start), None) => {
            let sequence_marker_field = {
                let trimmed = lines[start].trim_start();
                lines[start].len() - trimmed.len() == item_indent && trimmed.starts_with("- ")
            };
            let end = link_field_end(lines, start, range.end, item_indent);
            if sequence_marker_field {
                lines.splice(start..end, [format!("{}-", " ".repeat(item_indent))]);
            } else {
                lines.drain(start..end);
            }
        }
        (None, Some(value)) => {
            let insert_at = insertion_index(lines, range, item_indent, "reason");
            let field_indent = item_indent + 2;
            lines.insert(
                insert_at,
                format!(
                    "{}reason: {}",
                    " ".repeat(field_indent),
                    format_yaml_quoted_string(value)
                ),
            );
        }
        (None, None) => {}
    }
}

fn link_field_end(lines: &[String], start: usize, range_end: usize, item_indent: usize) -> usize {
    let field_indent = item_indent + 2;
    lines
        .iter()
        .enumerate()
        .take(range_end)
        .skip(start + 1)
        .find_map(|(index, line)| {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            (!trimmed.is_empty() && indent <= field_indent).then_some(index)
        })
        .unwrap_or(range_end)
}

fn update_optional_i64_field(
    lines: &mut Vec<String>,
    range: std::ops::Range<usize>,
    item_indent: usize,
    field: &str,
    value: Option<i64>,
) {
    update_optional_field(
        lines,
        range,
        item_indent,
        field,
        value.map(|value| value.to_string()),
    );
}

fn update_optional_field(
    lines: &mut Vec<String>,
    range: std::ops::Range<usize>,
    item_indent: usize,
    field: &str,
    value: Option<String>,
) {
    let field_index = range
        .clone()
        .find(|index| link_field_line(&lines[*index], item_indent, field));
    match (field_index, value) {
        (Some(index), Some(value)) => lines[index] = replace_yaml_value(&lines[index], &value),
        (Some(index), None) => {
            lines.remove(index);
        }
        (None, Some(value)) => {
            let insert_at = insertion_index(lines, range, item_indent, field);
            let field_indent = item_indent + 2;
            lines.insert(
                insert_at,
                format!("{}{}: {}", " ".repeat(field_indent), field, value),
            );
        }
        (None, None) => {}
    }
}

fn insertion_index(
    lines: &[String],
    range: std::ops::Range<usize>,
    item_indent: usize,
    field: &str,
) -> usize {
    let after = match field {
        "pathHint" => [Some("to"), None, None],
        "reason" => [Some("pathHint"), Some("to"), None],
        "order" => [Some("reason"), Some("pathHint"), Some("to")],
        _ => [None, None, None],
    };
    after
        .into_iter()
        .flatten()
        .find_map(|after| {
            range
                .clone()
                .find(|index| link_field_line(&lines[*index], item_indent, after))
                .map(|index| index + 1)
        })
        .unwrap_or(range.end)
}

fn link_range_from_start(
    lines: &[String],
    start: usize,
    item_indent: usize,
) -> std::ops::Range<usize> {
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if indent == item_indent && is_sequence_item_start(trimmed) {
                return Some(index);
            }
            if indent < item_indent && !trimmed.is_empty() && !trimmed.starts_with('#') {
                return Some(index);
            }
            None
        })
        .unwrap_or(lines.len());
    start..end
}

fn link_field_line(line: &str, item_indent: usize, field: &str) -> bool {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent == item_indent {
        let Some(rest) = trimmed.strip_prefix("- ") else {
            return false;
        };
        return field_line(rest, field);
    }
    indent == item_indent + 2 && field_line(trimmed, field)
}

fn is_sequence_item_start(trimmed: &str) -> bool {
    trimmed == "-" || trimmed.starts_with("- ")
}

fn field_line(trimmed: &str, field: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix(field) else {
        return false;
    };
    rest.starts_with(':')
}

fn replace_yaml_value(line: &str, value: &str) -> String {
    let Some((prefix, current_value)) = line.split_once(':') else {
        return line.to_string();
    };
    let comment_suffix = yaml_comment_suffix(current_value);
    format!("{prefix}: {value}{comment_suffix}")
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

fn format_link_lines(link: &Link, item_indent: usize) -> Vec<String> {
    let field_indent = item_indent + 2;
    let mut lines = vec![
        format!(
            "{}- rel: {}",
            " ".repeat(item_indent),
            format_yaml_string(&link.rel)
        ),
        format!(
            "{}to: {}",
            " ".repeat(field_indent),
            format_yaml_string(&link.to)
        ),
    ];
    if let Some(path_hint) = &link.path_hint {
        lines.push(format!(
            "{}pathHint: {}",
            " ".repeat(field_indent),
            format_yaml_string(path_hint)
        ));
    }
    if let Some(reason) = &link.reason {
        lines.push(format!(
            "{}reason: {}",
            " ".repeat(field_indent),
            format_yaml_quoted_string(reason)
        ));
    }
    if let Some(order) = link.order {
        lines.push(format!("{}order: {order}", " ".repeat(field_indent)));
    }
    lines
}

fn format_yaml_string(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '/' | '.' | '_' | '-'))
    {
        return value.to_string();
    }
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn format_yaml_quoted_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string cannot fail")
}

fn is_links_header(trimmed: &str) -> bool {
    trimmed.strip_prefix("links:").is_some()
}

fn is_plain_links_header(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("links:") else {
        return false;
    };
    rest.trim().is_empty() || rest.trim_start().starts_with('#')
}

fn is_empty_links_header(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("links:") else {
        return false;
    };
    rest.trim_start().starts_with("[]")
}

fn write_sidecar_text(root: &Path, sidecar_path: &str, text: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    fs::write(root.join(sidecar_path), text)
        .with_context(|| format!("failed to write {}", display_path(&root.join(sidecar_path))))?;
    Ok(())
}

fn validate_link(context: &EditContext, link: &Link) -> Result<String> {
    validate_link_reason(link.reason.as_deref()).map_err(anyhow::Error::msg)?;
    if link.rel.trim().is_empty() {
        anyhow::bail!("link relation must not be empty");
    }
    if !context.known_relations.is_empty() && !context.known_relations.contains(&link.rel) {
        anyhow::bail!("unknown relation {}", link.rel);
    }

    let target_path = match parse_locator(&link.to).map_err(anyhow::Error::msg)? {
        Locator::Id(id) => context
            .id_to_path
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unresolved id locator: {}", link.to))?,
        Locator::Path(_) => anyhow::bail!("link target must use an id: locator"),
    };

    Ok(target_path)
}

fn unique_link_index(links: &[Link], rel: &str, to: &str) -> Result<usize> {
    let matches = links
        .iter()
        .enumerate()
        .filter_map(|(index, link)| same_link_target(link, rel, to).then_some(index))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => anyhow::bail!("link not found: {rel}:{to}"),
        _ => anyhow::bail!("link match is ambiguous: {rel}:{to}"),
    }
}

fn same_link_target(link: &Link, rel: &str, to: &str) -> bool {
    link.rel == rel && link.to == to
}
