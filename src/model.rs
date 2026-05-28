use std::collections::BTreeMap;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

pub const CONFIG_PATH: &str = ".relaygraph.yaml";
pub const DEFAULT_SIDECAR_SUFFIX: &str = ".relaygraph.yaml";
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;
pub const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    #[serde(default, deserialize_with = "optional_no_null")]
    pub schema_version: Option<u32>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub use_git_ignore: Option<bool>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub sidecar_suffix: Option<String>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub plugins: Option<Vec<String>>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub exclude: Option<Vec<String>>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub require_sidecar: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: Some(SUPPORTED_SCHEMA_VERSION),
            use_git_ignore: Some(true),
            sidecar_suffix: Some(DEFAULT_SIDECAR_SUFFIX.to_string()),
            plugins: Some(vec!["relaygraph/plugins/feature-trace.yaml".to_string()]),
            exclude: Some(vec!["._relaygraph/**".to_string()]),
            require_sidecar: Some(Vec::new()),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sidecar {
    #[serde(default, deserialize_with = "optional_no_null")]
    pub schema_version: Option<u32>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub kind: Option<String>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Link {
    pub rel: String,
    pub to: String,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub order: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plugin {
    #[serde(default, deserialize_with = "optional_no_null")]
    pub schema_version: Option<u32>,
    pub name: String,
    #[serde(default)]
    pub resource_kinds: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default, deserialize_with = "optional_no_null")]
    pub traversal: Option<Traversal>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub when: String,
    #[serde(default)]
    pub require_any_outgoing: Vec<String>,
    #[serde(default)]
    pub require_reachable_kinds: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Traversal {
    #[serde(default)]
    pub start_kinds: Vec<String>,
    #[serde(default)]
    pub relation_order: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: &'static str,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug)]
pub struct Resource {
    pub path: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub sidecar: Option<String>,
    pub metadata: BTreeMap<String, serde_yaml::Value>,
    pub links: Vec<ResolvedLink>,
}

#[derive(Debug, Clone)]
pub struct ResolvedLink {
    pub rel: String,
    pub to: String,
    pub target_path: Option<String>,
    pub target_id: Option<String>,
    pub order: Option<i64>,
}

#[derive(Debug)]
pub enum Locator {
    Id(String),
    Path(String),
}

#[derive(Debug)]
pub struct BuildResult {
    pub resources: Vec<Resource>,
    pub diagnostics: Vec<Diagnostic>,
    pub plugins: Vec<Plugin>,
}

fn optional_no_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = serde_yaml::Value::deserialize(deserializer)?;
    match value {
        serde_yaml::Value::Null => Err(serde::de::Error::custom(
            "explicit null is not allowed; omit the field instead",
        )),
        value => T::deserialize(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}
