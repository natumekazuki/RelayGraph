use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::locator::parse_locator;
use crate::model::{Direction, Locator, Plugin, Resource};
use crate::plugin::build_relation_rank;
use crate::util::normalize_repo_path_strict;

struct TraceEdge {
    target_path: String,
    target_locator: String,
    rel: String,
    order: Option<i64>,
    relation_rank: usize,
    traversal: TraceTraversal,
    from: String,
    to: String,
}

struct PendingTrace {
    path: String,
    depth: usize,
    via: Option<TraceVia>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceResult {
    pub start: TraceStart,
    pub direction: Direction,
    pub nodes: Vec<TraceNode>,
}

impl TraceResult {
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|node| node.path.as_str())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceStart {
    pub locator: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceNode {
    pub path: String,
    pub depth: usize,
    pub via: Option<TraceVia>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceVia {
    pub traversal: TraceTraversal,
    pub rel: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceTraversal {
    Outgoing,
    Incoming,
}

pub fn trace_from(
    resources: &[Resource],
    plugins: &[Plugin],
    from: &str,
    direction: Direction,
) -> Result<TraceResult> {
    let relation_rank = build_relation_rank(plugins);
    let by_path = resources
        .iter()
        .map(|resource| (resource.path.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let by_id = resources
        .iter()
        .filter_map(|resource| {
            resource
                .id
                .as_deref()
                .map(|id| (id, resource.path.as_str()))
        })
        .collect::<BTreeMap<_, _>>();

    let start_path = match parse_locator(from).map_err(anyhow::Error::msg)? {
        Locator::Id(id) => by_id
            .get(id.as_str())
            .copied()
            .map(str::to_string)
            .with_context(|| format!("unknown start id: {id}"))?,
        Locator::Path(path) => {
            let path = normalize_repo_path_strict(&path).map_err(anyhow::Error::msg)?;
            if !by_path.contains_key(path.as_str()) {
                anyhow::bail!("unknown start path: {path}");
            }
            path
        }
    };

    let mut visited = BTreeSet::new();
    let mut pending = vec![PendingTrace {
        path: start_path.clone(),
        depth: 0,
        via: None,
    }];
    let mut ordered = Vec::new();

    while let Some(current) = pending.pop() {
        let path = current.path;
        if !visited.insert(path.clone()) {
            continue;
        }
        ordered.push(TraceNode {
            path: path.clone(),
            depth: current.depth,
            via: current.via,
        });

        let Some(resource) = by_path.get(path.as_str()) else {
            continue;
        };
        let mut next = Vec::new();
        if matches!(direction, Direction::Outgoing | Direction::Both) {
            next.extend(resource.links.iter().filter_map(|link| {
                link.target_path.as_ref().map(|target_path| TraceEdge {
                    target_path: target_path.clone(),
                    target_locator: link.to.clone(),
                    rel: link.rel.clone(),
                    order: link.order,
                    relation_rank: relation_rank
                        .get(link.rel.as_str())
                        .copied()
                        .unwrap_or(usize::MAX),
                    traversal: TraceTraversal::Outgoing,
                    from: resource.path.clone(),
                    to: target_path.clone(),
                })
            }));
        }
        if matches!(direction, Direction::Incoming | Direction::Both) {
            next.extend(resources.iter().flat_map(|source| {
                source
                    .links
                    .iter()
                    .filter(|link| link.target_path.as_deref() == Some(path.as_str()))
                    .map(|link| TraceEdge {
                        target_path: source.path.clone(),
                        target_locator: format!("path:{}", source.path),
                        rel: link.rel.clone(),
                        order: link.order,
                        relation_rank: relation_rank
                            .get(link.rel.as_str())
                            .copied()
                            .unwrap_or(usize::MAX),
                        traversal: TraceTraversal::Incoming,
                        from: source.path.clone(),
                        to: path.clone(),
                    })
            }));
        }
        next.sort_by(|left, right| {
            (
                left.order.unwrap_or(i64::MAX),
                left.relation_rank,
                &left.rel,
                &left.target_locator,
            )
                .cmp(&(
                    right.order.unwrap_or(i64::MAX),
                    right.relation_rank,
                    &right.rel,
                    &right.target_locator,
                ))
        });
        next.reverse();
        pending.extend(next.into_iter().map(|edge| PendingTrace {
            path: edge.target_path,
            depth: current.depth + 1,
            via: Some(TraceVia {
                traversal: edge.traversal,
                rel: edge.rel,
                from: edge.from,
                to: edge.to,
            }),
        }));
    }

    Ok(TraceResult {
        start: TraceStart {
            locator: from.to_string(),
            path: start_path,
        },
        direction,
        nodes: ordered,
    })
}
