use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::model::{AcknowledgedRevisions, BuildResult};

pub const RELATION_REVIEW_REQUIRED: &str = "relation-review-required";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FreshnessState {
    SourceChanged,
    TargetChanged,
    BothChanged,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationFreshnessDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
    pub state: FreshnessState,
    pub relation: FreshnessRelation,
    pub source: FreshnessEndpoint,
    pub target: FreshnessEndpoint,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshnessRelation {
    pub rel: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshnessEndpoint {
    pub id: Option<String>,
    pub path: String,
    pub acknowledged_revision: String,
    pub current_revision: String,
}

pub fn validate_relation_freshness(
    root: &Path,
    graph: &BuildResult,
) -> Result<Vec<RelationFreshnessDiagnostic>> {
    let mut diagnostics = Vec::new();
    for source in &graph.resources {
        for link in &source.links {
            let Some(acknowledged) = &link.acknowledged else {
                continue;
            };
            let Some(target_path) = &link.target_path else {
                continue;
            };

            let source_revision = fingerprint_file(&root.join(&source.path))?;
            let target_revision = fingerprint_file(&root.join(target_path))?;
            let source_changed = source_revision != acknowledged.source_revision;
            let target_changed = target_revision != acknowledged.target_revision;
            let state = match (source_changed, target_changed) {
                (false, false) => continue,
                (true, false) => FreshnessState::SourceChanged,
                (false, true) => FreshnessState::TargetChanged,
                (true, true) => FreshnessState::BothChanged,
            };
            let sidecar_path = source
                .sidecar
                .clone()
                .unwrap_or_else(|| source.path.clone());
            diagnostics.push(RelationFreshnessDiagnostic {
                code: RELATION_REVIEW_REQUIRED,
                path: sidecar_path,
                message: freshness_message(state),
                state,
                relation: FreshnessRelation {
                    rel: link.rel.clone(),
                    to: link.to.clone(),
                },
                source: FreshnessEndpoint {
                    id: source.id.clone(),
                    path: source.path.clone(),
                    acknowledged_revision: acknowledged.source_revision.clone(),
                    current_revision: source_revision,
                },
                target: FreshnessEndpoint {
                    id: link.target_id.clone(),
                    path: target_path.clone(),
                    acknowledged_revision: acknowledged.target_revision.clone(),
                    current_revision: target_revision,
                },
            });
        }
    }
    Ok(diagnostics)
}

pub fn print_relation_freshness(diagnostics: &[RelationFreshnessDiagnostic]) {
    for diagnostic in diagnostics {
        println!(
            "{} {}: {} --{}--> {} ({})",
            diagnostic.code,
            diagnostic.path,
            diagnostic.source.path,
            diagnostic.relation.rel,
            diagnostic.target.path,
            diagnostic.message
        );
    }
}

pub fn acknowledged_revisions(
    root: &Path,
    source: &str,
    target: &str,
) -> Result<AcknowledgedRevisions> {
    Ok(AcknowledgedRevisions {
        source_revision: fingerprint_file(&root.join(source))?,
        target_revision: fingerprint_file(&root.join(target))?,
    })
}

pub fn fingerprint_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| {
        format!(
            "failed to read resource for fingerprint: {}",
            path.display()
        )
    })?;
    let normalized;
    let input = if let Ok(text) = std::str::from_utf8(&bytes) {
        normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_bytes()
    } else {
        bytes.as_slice()
    };
    let digest = Sha256::digest(input);
    Ok(format!("sha256:{digest:x}"))
}

fn freshness_message(state: FreshnessState) -> String {
    match state {
        FreshnessState::SourceChanged => "source revision changed after acknowledgement",
        FreshnessState::TargetChanged => "target revision changed after acknowledgement",
        FreshnessState::BothChanged => "source and target revisions changed after acknowledgement",
    }
    .to_string()
}
