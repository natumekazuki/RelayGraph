use std::collections::BTreeMap;

use serde::Serialize;

use crate::model::{BuildResult, Diagnostic, Traversal};
use crate::plugin::build_relation_rank;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportGraph {
    pub resources: Vec<ExportResource>,
    pub diagnostics: Vec<Diagnostic>,
    pub plugins: Vec<ExportPlugin>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlugin {
    pub name: String,
    pub traversal: Option<Traversal>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResource {
    pub path: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub sidecar: Option<String>,
    pub metadata: BTreeMap<String, serde_yaml::Value>,
    pub links: Vec<ExportLink>,
    pub incoming_links: Vec<ExportIncomingLink>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportLink {
    pub rel: String,
    pub to: String,
    pub target_path: Option<String>,
    pub target_id: Option<String>,
    pub order: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportIncomingLink {
    pub from_path: String,
    pub from_id: Option<String>,
    pub rel: String,
    pub order: Option<i64>,
}

pub fn to_export(graph: BuildResult) -> ExportGraph {
    let relation_rank = build_relation_rank(&graph.plugins);
    let mut incoming_by_path = BTreeMap::<String, Vec<ExportIncomingLink>>::new();
    for resource in &graph.resources {
        for link in &resource.links {
            if let Some(target_path) = &link.target_path {
                incoming_by_path
                    .entry(target_path.clone())
                    .or_default()
                    .push(ExportIncomingLink {
                        from_path: resource.path.clone(),
                        from_id: resource.id.clone(),
                        rel: link.rel.clone(),
                        order: link.order,
                    });
            }
        }
    }
    for incoming in incoming_by_path.values_mut() {
        incoming.sort_by(|left, right| {
            (
                left.order.unwrap_or(i64::MAX),
                relation_rank
                    .get(left.rel.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                &left.rel,
                &left.from_path,
            )
                .cmp(&(
                    right.order.unwrap_or(i64::MAX),
                    relation_rank
                        .get(right.rel.as_str())
                        .copied()
                        .unwrap_or(usize::MAX),
                    &right.rel,
                    &right.from_path,
                ))
        });
    }

    ExportGraph {
        resources: graph
            .resources
            .into_iter()
            .map(|resource| {
                let incoming_links = incoming_by_path.remove(&resource.path).unwrap_or_default();
                ExportResource {
                    path: resource.path,
                    id: resource.id,
                    kind: resource.kind,
                    sidecar: resource.sidecar,
                    metadata: resource.metadata,
                    links: resource
                        .links
                        .into_iter()
                        .map(|link| ExportLink {
                            rel: link.rel,
                            to: link.to,
                            target_path: link.target_path,
                            target_id: link.target_id,
                            order: link.order,
                        })
                        .collect(),
                    incoming_links,
                }
            })
            .collect(),
        diagnostics: graph.diagnostics,
        plugins: graph
            .plugins
            .into_iter()
            .map(|plugin| ExportPlugin {
                name: plugin.name,
                traversal: plugin.traversal,
            })
            .collect(),
    }
}
