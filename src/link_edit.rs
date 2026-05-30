use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::diagnostic::diagnostics_to_message;
use crate::generate::GenerateLink;
use crate::graph::build_graph;
use crate::locator::parse_locator;
use crate::model::{Config, Diagnostic, Link, Locator, Sidecar};
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
    pub order: Option<i64>,
    pub clear_order: bool,
}

pub fn add_link(root: &Path, config: &Config, options: AddLinkOptions) -> Result<String> {
    let context = load_edit_context(root, config, &options.common.source)?;
    let mut link = Link {
        rel: options.link.rel,
        to: options.link.to,
        path_hint: None,
        order: options.order,
    };
    let target_path = validate_link(&context, &link)?;
    if options.path_hint {
        link.path_hint = Some(target_path);
    }

    let text = read_sidecar_text(root, &context.sidecar_path)?;
    let sidecar = parse_sidecar(&text, &context.sidecar_path)?;
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
    let updated = apply_link_add(&text, &link)
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
        && options.order.is_none()
        && !options.clear_order
    {
        anyhow::bail!(
            "update requires --new, --path-hint, --clear-path-hint, --order, or --clear-order"
        );
    }
    if options.path_hint && options.clear_path_hint {
        anyhow::bail!("--path-hint and --clear-path-hint cannot be used together");
    }
    if options.order.is_some() && options.clear_order {
        anyhow::bail!("--order and --clear-order cannot be used together");
    }

    let context = load_edit_context(root, config, &options.common.source)?;
    let text = read_sidecar_text(root, &context.sidecar_path)?;
    let mut sidecar = parse_sidecar(&text, &context.sidecar_path)?;
    let index = unique_link_index(&sidecar.links, &options.current.rel, &options.current.to)?;

    let target_changed = options.new_link.is_some();
    if let Some(new_link) = options.new_link {
        sidecar.links[index].rel = new_link.rel;
        sidecar.links[index].to = new_link.to;
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

    let link = &sidecar.links[index];
    if sidecar
        .links
        .iter()
        .enumerate()
        .any(|(other_index, existing)| {
            other_index != index && same_link_target(existing, &link.rel, &link.to)
        })
    {
        anyhow::bail!(
            "updated link would duplicate existing link in {}: {}:{}",
            context.sidecar_path,
            link.rel,
            link.to
        );
    }

    let updated = apply_link_update(&text, index, link)
        .with_context(|| format!("failed to update links in {}", context.sidecar_path))?;
    write_sidecar_text(
        root,
        &context.sidecar_path,
        &updated,
        options.common.dry_run,
    )?;
    Ok(context.sidecar_path)
}

struct EditContext {
    sidecar_path: String,
    known_relations: Vec<String>,
    id_to_path: BTreeMap<String, String>,
}

fn load_edit_context(root: &Path, config: &Config, source: &str) -> Result<EditContext> {
    let source_id = match parse_locator(source).map_err(anyhow::Error::msg)? {
        Locator::Id(id) => id,
        Locator::Path(_) => anyhow::bail!("link edit source must use an id: locator"),
    };
    let graph = build_graph(root, config)?;
    let blocking = link_edit_blocking_diagnostics(&graph.diagnostics);
    if !blocking.is_empty() {
        anyhow::bail!("{}", diagnostics_to_message(&blocking));
    }

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
        sidecar_path,
        known_relations,
        id_to_path,
    })
}

fn read_sidecar_text(root: &Path, sidecar_path: &str) -> Result<String> {
    fs::read_to_string(root.join(sidecar_path))
        .with_context(|| format!("failed to read sidecar {sidecar_path}"))
}

fn parse_sidecar(text: &str, sidecar_path: &str) -> Result<Sidecar> {
    serde_yaml::from_str(text).with_context(|| format!("failed to parse sidecar {sidecar_path}"))
}

fn link_edit_blocking_diagnostics(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| !is_link_edit_repairable_diagnostic(diagnostic.code))
        .cloned()
        .collect()
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
    update_optional_i64_field(lines, range, item_indent, "order", link.order);
    Ok(())
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
    let after = if field == "pathHint" {
        "to"
    } else {
        "pathHint"
    };
    range
        .clone()
        .find(|index| link_field_line(&lines[*index], item_indent, after))
        .map_or(range.end, |index| index + 1)
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
