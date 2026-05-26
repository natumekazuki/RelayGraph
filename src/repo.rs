use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::model::Diagnostic;
use crate::util::{display_path, is_repo_boundary_link, normalize_repo_path};

pub fn list_repo_files(root: &Path, use_git_ignore: bool) -> Result<Vec<String>> {
    list_repo_files_inner(root, use_git_ignore, None)
}

pub fn list_repo_files_with_diagnostics(
    root: &Path,
    use_git_ignore: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<String>> {
    list_repo_files_inner(root, use_git_ignore, Some(diagnostics))
}

pub fn is_git_ignored(root: &Path, repo_path: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["check-ignore", "-q", "--", repo_path])
        .current_dir(root)
        .output()
        .context("failed to run git check-ignore")?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!(
            "git check-ignore failed for {repo_path}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    }
}

fn list_repo_files_inner(
    root: &Path,
    use_git_ignore: bool,
    mut diagnostics: Option<&mut Vec<Diagnostic>>,
) -> Result<Vec<String>> {
    if use_git_ignore {
        let output = Command::new("git")
            .args([
                "-c",
                "core.quotePath=false",
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ])
            .current_dir(root)
            .output()
            .context("failed to run git ls-files")?;
        if output.status.success() {
            return Ok(output
                .stdout
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
                .map(String::from_utf8_lossy)
                .map(normalize_repo_path)
                .filter_map(
                    |path| match is_repo_file_or_boundary_declaration(root, &path) {
                        Ok(true) => Some(Ok(path)),
                        Ok(false) => None,
                        Err(error) => {
                            if let Some(diagnostics) = diagnostics.as_deref_mut() {
                                diagnostics.push(Diagnostic {
                                    code: "schema-error",
                                    path: Some(path),
                                    message: format!("failed to inspect repo path: {error:#}"),
                                });
                                None
                            } else {
                                Some(Err(error))
                            }
                        }
                    },
                )
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|path| !path.is_empty())
                .collect());
        }
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read {}", display_path(current)))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name.to_string_lossy() == ".git" {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", display_path(&path)))?;
        if is_repo_boundary_link(&metadata) {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            files.push(normalize_repo_path(relative.to_string_lossy()));
            continue;
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            files.push(normalize_repo_path(relative.to_string_lossy()));
        }
    }
    Ok(())
}

fn is_repo_file_or_boundary_declaration(root: &Path, repo_path: &str) -> Result<bool> {
    let relative = Path::new(repo_path);
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", display_path(&current)));
            }
        };
        let is_last = index + 1 == components.len();
        if is_repo_boundary_link(&metadata) {
            return Ok(is_last);
        }
        if is_last {
            return Ok(metadata.is_file());
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn git_ignore_mode_fails_outside_git_repository() {
        let root = temp_root("relaygraph-no-git");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "a").unwrap();

        let result = list_repo_files(&root, true);

        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn git_ignore_mode_preserves_non_ascii_paths() {
        let root = temp_root("relaygraph-non-ascii");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("日本.md"), "# 日本\n").unwrap();
        let init = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(init.status.success());
        let config = Command::new("git")
            .args(["config", "core.quotePath", "true"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(config.status.success());

        let files = list_repo_files(&root, true).unwrap();

        assert!(files.contains(&"日本.md".to_string()));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn git_ignore_mode_skips_files_under_junction() {
        let root = temp_root("relaygraph-junction-root");
        let outside = temp_root("relaygraph-junction-outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("outside.md"), "# Outside\n").unwrap();
        let init = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(init.status.success());
        let link = root.join("linked");
        let junction = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().unwrap(),
                outside.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(junction.status.success());

        let files = list_repo_files(&root, true).unwrap();

        assert!(!files.contains(&"linked/outside.md".to_string()));
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
    }
}
