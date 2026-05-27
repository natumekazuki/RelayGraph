use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::util::{display_path, is_repo_boundary_link};

const SKILL_NAME: &str = "relaygraph";

struct SkillFile {
    path: &'static str,
    contents: &'static str,
}

const SKILL_FILES: &[SkillFile] = &[
    SkillFile {
        path: "SKILL.md",
        contents: include_str!("../.agents/skills/relaygraph/SKILL.md"),
    },
    SkillFile {
        path: "agents/openai.yaml",
        contents: include_str!("../.agents/skills/relaygraph/agents/openai.yaml"),
    },
    SkillFile {
        path: "references/cli.md",
        contents: include_str!("../.agents/skills/relaygraph/references/cli.md"),
    },
    SkillFile {
        path: "references/repository-rules.md",
        contents: include_str!("../.agents/skills/relaygraph/references/repository-rules.md"),
    },
    SkillFile {
        path: "references/sidecar-v1.md",
        contents: include_str!("../.agents/skills/relaygraph/references/sidecar-v1.md"),
    },
];

pub fn install_skill(skills_dir: &Path) -> Result<PathBuf> {
    let target = skills_dir.join(SKILL_NAME);
    validate_existing_skill_target(&target)?;
    fs::create_dir_all(skills_dir)
        .with_context(|| format!("failed to create {}", display_path(skills_dir)))?;

    let temp = unique_child_path(skills_dir, ".relaygraph-install")?;
    fs::create_dir(&temp).with_context(|| format!("failed to create {}", display_path(&temp)))?;

    let result = write_skill_files(&temp).and_then(|()| replace_skill_dir(&target, &temp));
    if result.is_err() {
        let _ = cleanup_install_dir(&temp);
    }
    result?;

    Ok(target)
}

fn write_skill_files(target: &Path) -> Result<()> {
    for file in SKILL_FILES {
        let path = target.join(file.path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", display_path(parent)))?;
        }
        fs::write(&path, file.contents)
            .with_context(|| format!("failed to write {}", display_path(&path)))?;
    }
    Ok(())
}

fn replace_skill_dir(target: &Path, temp: &Path) -> Result<()> {
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .context("skill target must have a parent directory")?;
    let backup = unique_child_path(parent, ".relaygraph-backup")?;
    let had_existing = target.exists();

    if had_existing {
        fs::rename(target, &backup).with_context(|| {
            format!(
                "failed to move existing skill {} to backup {}",
                display_path(target),
                display_path(&backup)
            )
        })?;
    }

    match fs::rename(temp, target) {
        Ok(()) => {
            if had_existing {
                cleanup_install_dir(&backup).with_context(|| {
                    format!("failed to remove backup {}", display_path(&backup))
                })?;
            }
            Ok(())
        }
        Err(error) => {
            if had_existing {
                restore_backup(target, &backup, &error)?;
            }
            Err(error).with_context(|| {
                format!(
                    "failed to install skill {} from {}",
                    display_path(target),
                    display_path(temp)
                )
            })
        }
    }
}

fn restore_backup(target: &Path, backup: &Path, install_error: &std::io::Error) -> Result<()> {
    fs::rename(backup, target).with_context(|| {
        format!(
            "failed to restore previous skill {} after install failure ({install_error})",
            display_path(target)
        )
    })
}

fn cleanup_install_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!(
                "refusing to remove symlink or reparse point {}",
                display_path(path)
            );
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path)
                .with_context(|| format!("failed to remove {}", display_path(path)))?;
        }
        Ok(_) => {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", display_path(path)))?;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", display_path(path)));
        }
    }
    Ok(())
}

fn validate_existing_skill_target(target: &Path) -> Result<()> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!(
                "refusing to replace symlink or reparse point {}",
                display_path(target)
            );
        }
        Ok(metadata) if metadata.is_dir() => {
            ensure_existing_relaygraph_skill(target)?;
        }
        Ok(_) => {
            anyhow::bail!(
                "refusing to replace non-directory skill target {}",
                display_path(target)
            );
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", display_path(target)));
        }
    }
    Ok(())
}

fn unique_child_path(parent: &Path, prefix: &str) -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for counter in 0..1000 {
        let candidate = parent.join(format!("{prefix}-{}-{nanos}-{counter}", std::process::id()));
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(candidate),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", display_path(&candidate)));
            }
            Ok(_) => {}
        }
    }
    anyhow::bail!(
        "failed to allocate temporary install path under {}",
        display_path(parent)
    );
}

fn ensure_existing_relaygraph_skill(target: &Path) -> Result<()> {
    let skill_md = target.join("SKILL.md");
    let metadata = match fs::symlink_metadata(&skill_md) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            anyhow::bail!(
                "refusing to replace non-RelayGraph skill directory {}; SKILL.md is missing",
                display_path(target)
            );
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect existing skill {}",
                    display_path(&skill_md)
                )
            });
        }
    };
    if is_repo_boundary_link(&metadata) || !metadata.is_file() {
        anyhow::bail!(
            "refusing to replace non-RelayGraph skill directory {}; SKILL.md is not a regular file",
            display_path(target)
        );
    }

    let content = fs::read_to_string(&skill_md)
        .with_context(|| format!("failed to read {}", display_path(&skill_md)))?;
    if !has_relaygraph_skill_name(&content) {
        anyhow::bail!(
            "refusing to replace non-RelayGraph skill directory {}; SKILL.md name is not relaygraph",
            display_path(target)
        );
    }
    Ok(())
}

fn has_relaygraph_skill_name(content: &str) -> bool {
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return false;
    }

    for line in lines {
        if line == "---" {
            return false;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }
        return value.trim().trim_matches('"').trim_matches('\'') == SKILL_NAME;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_relaygraph_skill_frontmatter_name() {
        assert!(has_relaygraph_skill_name(
            "---\nname: relaygraph\ndescription: x\n---\n"
        ));
        assert!(has_relaygraph_skill_name(
            "---\nname: \"relaygraph\"\ndescription: x\n---\n"
        ));
        assert!(!has_relaygraph_skill_name(
            "---\nname: other\ndescription: x\n---\n"
        ));
        assert!(!has_relaygraph_skill_name("name: relaygraph\n"));
    }

    #[test]
    fn replace_restores_previous_skill_when_new_dir_move_fails() {
        let root = std::env::temp_dir().join(format!(
            "relaygraph-skill-restore-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("relaygraph");
        fs::create_dir_all(&target).unwrap();
        fs::write(
            target.join("SKILL.md"),
            "---\nname: relaygraph\ndescription: old\n---\n",
        )
        .unwrap();
        fs::write(target.join("old.txt"), "old\n").unwrap();

        let missing_temp = root.join("missing-temp");
        let result = replace_skill_dir(&target, &missing_temp);

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "---\nname: relaygraph\ndescription: old\n---\n"
        );
        assert_eq!(fs::read_to_string(target.join("old.txt")).unwrap(), "old\n");

        let _ = fs::remove_dir_all(root);
    }
}
