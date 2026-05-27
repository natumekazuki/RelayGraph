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
            ensure_existing_relaygraph_skill(target)?;
            fs::remove_dir_all(target)
                .with_context(|| format!("failed to remove {}", display_path(target)))?;
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
}
