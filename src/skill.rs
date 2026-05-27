use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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
    remove_existing_skill(&target)?;
    fs::create_dir_all(&target)
        .with_context(|| format!("failed to create {}", display_path(&target)))?;

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

    Ok(target)
}

fn remove_existing_skill(target: &Path) -> Result<()> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if is_repo_boundary_link(&metadata) => {
            anyhow::bail!(
                "refusing to replace symlink or reparse point {}",
                display_path(target)
            );
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(target)
                .with_context(|| format!("failed to remove {}", display_path(target)))?;
        }
        Ok(_) => {
            fs::remove_file(target)
                .with_context(|| format!("failed to remove {}", display_path(target)))?;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", display_path(target)));
        }
    }
    Ok(())
}
