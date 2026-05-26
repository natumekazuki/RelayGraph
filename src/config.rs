use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{Config, Diagnostic, CONFIG_PATH, DEFAULT_SIDECAR_SUFFIX};
use crate::repo::{is_git_ignored, is_git_repository};
use crate::util::is_repo_boundary_link;

pub fn load_config(root: &Path) -> Result<Config> {
    let path = root.join(CONFIG_PATH);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if is_repo_boundary_link(&metadata) {
                anyhow::bail!(".relaygraph.yaml must not be a symlink");
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Config::default()),
        Err(error) => return Err(error).context("failed to inspect .relaygraph.yaml"),
    };

    let text = fs::read_to_string(&path).context("failed to read .relaygraph.yaml")?;
    let config: Config = serde_yaml::from_str(&text).context("failed to parse .relaygraph.yaml")?;
    let config = apply_config_defaults(config);
    if config.use_git_ignore.unwrap_or(true)
        && is_git_repository(root)
        && is_git_ignored(root, CONFIG_PATH)?
    {
        anyhow::bail!(".relaygraph.yaml must be part of Git-backed repository discovery");
    }
    Ok(config)
}

fn apply_config_defaults(mut config: Config) -> Config {
    let default = Config::default();
    if config.schema_version.is_none() {
        config.schema_version = default.schema_version;
    }
    if config.use_git_ignore.is_none() {
        config.use_git_ignore = default.use_git_ignore;
    }
    if config.sidecar_suffix.is_none() {
        config.sidecar_suffix = default.sidecar_suffix;
    }
    if config.plugins.is_none() {
        config.plugins = default.plugins;
    }
    if config.exclude.is_none() {
        config.exclude = default.exclude;
    }
    if config.require_sidecar.is_none() {
        config.require_sidecar = default.require_sidecar;
    }
    config
}

pub fn sidecar_suffix(config: &Config) -> String {
    match config.sidecar_suffix.as_deref() {
        Some(suffix) if suffix.trim().is_empty() => DEFAULT_SIDECAR_SUFFIX.to_string(),
        Some(suffix) => suffix.to_string(),
        None => DEFAULT_SIDECAR_SUFFIX.to_string(),
    }
}

pub fn validate_config_values(config: &Config, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(suffix) = config.sidecar_suffix.as_deref() {
        if !is_valid_sidecar_suffix(suffix) {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(CONFIG_PATH.to_string()),
                message: "sidecarSuffix must be a non-empty filename suffix without path separators or parent traversal".to_string(),
            });
        }
    }

    validate_non_blank_list("plugins", config.plugins.as_deref(), diagnostics);
    validate_non_blank_list("exclude", config.exclude.as_deref(), diagnostics);
    validate_non_blank_list(
        "requireSidecar",
        config.require_sidecar.as_deref(),
        diagnostics,
    );
}

fn validate_non_blank_list(
    field: &str,
    values: Option<&[String]>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for value in values.into_iter().flatten() {
        if value.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "schema-error",
                path: Some(CONFIG_PATH.to_string()),
                message: format!("{field} entries must not be empty or whitespace"),
            });
        }
    }
}

fn is_valid_sidecar_suffix(suffix: &str) -> bool {
    const RESERVED_FILENAME_CHARACTERS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    !suffix.trim().is_empty()
        && !suffix
            .chars()
            .last()
            .is_some_and(|character| character == '.' || character.is_whitespace())
        && !suffix
            .chars()
            .any(|character| RESERVED_FILENAME_CHARACTERS.contains(&character))
        && !suffix.split('.').any(|component| component == "..")
        && !suffix.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sidecar_suffix_is_applied() {
        assert_eq!(sidecar_suffix(&Config::default()), ".relaygraph.yaml");
    }

    #[test]
    fn whitespace_only_sidecar_suffix_falls_back_to_default() {
        let config = Config {
            sidecar_suffix: Some(" ".to_string()),
            ..Config::default()
        };

        assert_eq!(sidecar_suffix(&config), ".relaygraph.yaml");
    }

    #[test]
    fn config_path_read_errors_are_not_treated_as_missing() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "relaygraph-config-read-error-{}-{nanos}",
            std::process::id(),
        ));
        fs::create_dir_all(root.join(CONFIG_PATH)).unwrap();

        let result = load_config(&root);

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validates_whitespace_only_config_strings() {
        let config = Config {
            sidecar_suffix: Some(" ".to_string()),
            plugins: Some(vec![" ".to_string()]),
            exclude: Some(vec![" ".to_string()]),
            require_sidecar: Some(vec![" ".to_string()]),
            ..Config::default()
        };
        let mut diagnostics = Vec::new();

        validate_config_values(&config, &mut diagnostics);

        assert_eq!(diagnostics.len(), 4);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "schema-error"));
    }

    #[test]
    fn validates_sidecar_suffix_as_filename_suffix() {
        for suffix in [
            "dir/.relaygraph.yaml",
            "dir\\.relaygraph.yaml",
            "../x",
            ".rg..x",
            ":rg.yaml",
            "*.yaml",
            "?.yaml",
            "|.yaml",
            ".rg.",
            ".rg ",
        ] {
            let config = Config {
                sidecar_suffix: Some(suffix.to_string()),
                ..Config::default()
            };
            let mut diagnostics = Vec::new();

            validate_config_values(&config, &mut diagnostics);

            assert_eq!(diagnostics.len(), 1, "suffix should be rejected: {suffix}");
            assert!(diagnostics[0]
                .message
                .contains("without path separators or parent traversal"));
        }
    }
}
