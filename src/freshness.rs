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
    LinkChanged,
    SourceAndLinkChanged,
    TargetAndLinkChanged,
    AllChanged,
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
    pub acknowledged_revision: Option<String>,
    pub current_revision: String,
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
            let link_revision =
                fingerprint_link_revision(&link.rel, &link.to, link.reason.as_deref());
            let source_changed = source_revision != acknowledged.source_revision;
            let target_changed = target_revision != acknowledged.target_revision;
            let link_changed = acknowledged
                .link_revision
                .as_ref()
                .is_some_and(|acknowledged| acknowledged != &link_revision);
            let state = match (source_changed, target_changed, link_changed) {
                (false, false, false) => continue,
                (true, false, false) => FreshnessState::SourceChanged,
                (false, true, false) => FreshnessState::TargetChanged,
                (true, true, false) => FreshnessState::BothChanged,
                (false, false, true) => FreshnessState::LinkChanged,
                (true, false, true) => FreshnessState::SourceAndLinkChanged,
                (false, true, true) => FreshnessState::TargetAndLinkChanged,
                (true, true, true) => FreshnessState::AllChanged,
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
                    acknowledged_revision: acknowledged.link_revision.clone(),
                    current_revision: link_revision,
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
    link_revision: Option<String>,
) -> Result<AcknowledgedRevisions> {
    Ok(AcknowledgedRevisions {
        source_revision: fingerprint_file(&root.join(source))?,
        target_revision: fingerprint_file(&root.join(target))?,
        link_revision,
    })
}

pub fn fingerprint_link_revision(rel: &str, to: &str, reason: Option<&str>) -> String {
    let mut payload = b"relaygraph-link-review-v1\0".to_vec();
    append_fingerprint_component(&mut payload, rel.as_bytes());
    append_fingerprint_component(&mut payload, to.as_bytes());
    match reason {
        Some(reason) => {
            payload.push(1);
            append_fingerprint_component(&mut payload, reason.as_bytes());
        }
        None => payload.push(0),
    }
    let digest = Sha256::digest(payload);
    format!("sha256:{digest:x}")
}

fn append_fingerprint_component(payload: &mut Vec<u8>, component: &[u8]) {
    payload.extend_from_slice(&(component.len() as u64).to_be_bytes());
    payload.extend_from_slice(component);
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
        FreshnessState::LinkChanged => "link revision changed after acknowledgement",
        FreshnessState::SourceAndLinkChanged => {
            "source and link revisions changed after acknowledgement"
        }
        FreshnessState::TargetAndLinkChanged => {
            "target and link revisions changed after acknowledgement"
        }
        FreshnessState::AllChanged => {
            "source, target, and link revisions changed after acknowledgement"
        }
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::fingerprint_link_revision;

    #[test]
    fn link_revision_separates_each_component_and_component_boundaries() {
        let baseline = fingerprint_link_revision("ab", "id:c", Some("reason"));
        assert_ne!(
            baseline,
            fingerprint_link_revision("xy", "id:c", Some("reason"))
        );
        assert_ne!(
            baseline,
            fingerprint_link_revision("ab", "id:d", Some("reason"))
        );
        assert_ne!(
            baseline,
            fingerprint_link_revision("ab", "id:c", Some("other"))
        );
        assert_ne!(baseline, fingerprint_link_revision("ab", "id:c", None));
        assert_ne!(
            fingerprint_link_revision("ab", "c", None),
            fingerprint_link_revision("a", "bc", None)
        );
    }
}
