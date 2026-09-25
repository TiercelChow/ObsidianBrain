use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::BrainError;
use crate::models::book_wiki::WikiSkill;

/// Materialize only the version-frozen, book-scoped text selected for this Run.
/// The Harness filesystem provider must not scan the user's own skill roots.
pub(super) fn materialize_skills(
    workspace: &Path,
    skills: &[WikiSkill],
) -> Result<Option<PathBuf>, BrainError> {
    if skills.is_empty() {
        return Ok(None);
    }
    let root = workspace.join("wiki-skills");
    std::fs::create_dir(&root)?;
    let mut names = HashSet::new();
    for skill in skills {
        let slug = skill.slug.as_str();
        if !valid_slug(slug) || !names.insert(slug) {
            return Err(BrainError::KnowledgeValidation(format!(
                "Harness Skill 标识无效或重复: {slug}"
            )));
        }
        let directory = root.join(slug);
        std::fs::create_dir(&directory)?;
        let description = serde_json::to_string(&skill.description)
            .map_err(|error| BrainError::Internal(format!("Skill 描述序列化失败: {error}")))?;
        let body = format!(
            "---\nname: {slug}\ndescription: {description}\n---\n\n{}\n",
            skill.instructions.trim()
        );
        std::fs::write(directory.join("SKILL.md"), body)?;
    }
    Ok(Some(root))
}

pub(super) fn skill_patch(root: &Path) -> Result<String, BrainError> {
    if !root.is_absolute() || !root.is_dir() {
        return Err(BrainError::KnowledgeValidation(
            "Harness Skill 目录必须是已存在的绝对目录".to_string(),
        ));
    }
    serde_json::to_string_pretty(&serde_json::json!([
        {
            "id": "skill-filesystem",
            "name": "@deepseek-ai/dsh-skill-filesystem",
            "disabled": false,
            "config": {
                "includeDefaultRoots": false,
                "customSkillDirs": [root],
                "watch": false
            }
        },
        {
            "id": "tool-skill",
            "name": "@deepseek-ai/dsh-tool-skill",
            "disabled": false
        }
    ]))
    .map_err(|error| BrainError::Internal(format!("Harness Skill Patch 生成失败: {error}")))
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn skill(slug: &str) -> WikiSkill {
        WikiSkill {
            id: format!("skill-{slug}"),
            slug: slug.to_string(),
            name: slug.to_string(),
            description: "只读研究\n说明".to_string(),
            source_type: "builtin".to_string(),
            status: "ready".to_string(),
            permissions: Vec::new(),
            requirements: Vec::new(),
            revision: 3,
            instructions: "# 工作方法\n按证据回答。".to_string(),
            enabled: true,
            usage_scope: "qa".to_string(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn test_materialize_skills_writes_scoped_skill_and_isolated_patch() {
        let workspace = tempfile::tempdir().unwrap();
        let root = materialize_skills(workspace.path(), &[skill("book-qa")])
            .unwrap()
            .unwrap();
        let body = std::fs::read_to_string(root.join("book-qa/SKILL.md")).unwrap();
        assert!(body.contains("name: book-qa"));
        assert!(body.contains("按证据回答"));
        let patch: serde_json::Value = serde_json::from_str(&skill_patch(&root).unwrap()).unwrap();
        assert_eq!(patch[0]["config"]["includeDefaultRoots"], false);
        assert_eq!(
            patch[0]["config"]["customSkillDirs"][0],
            root.to_string_lossy().as_ref()
        );
        assert_eq!(patch[1]["disabled"], false);
    }

    #[test]
    fn test_materialize_skills_rejects_path_escape_and_duplicates() {
        let workspace = tempfile::tempdir().unwrap();
        assert!(materialize_skills(workspace.path(), &[skill("../outside")]).is_err());
        assert!(
            materialize_skills(workspace.path(), &[skill("book-qa"), skill("book-qa")]).is_err()
        );
        assert!(!workspace.path().join("outside").exists());
    }

    #[test]
    #[ignore = "requires the pinned DeepSeek Harness CLI; run explicitly for integration verification"]
    fn test_real_harness_accepts_isolated_native_skill_patch() {
        let workspace = tempfile::tempdir().unwrap();
        let root = materialize_skills(workspace.path(), &[skill("book-qa")])
            .unwrap()
            .unwrap();
        let safety = workspace.path().join("safety.patch.yml");
        let native = workspace.path().join("skills.patch.json");
        std::fs::write(&safety, super::super::KNOWLEDGE_QA_HARNESS_PATCH).unwrap();
        std::fs::write(&native, skill_patch(&root).unwrap()).unwrap();
        let output = Command::new("npx")
            .args([
                "-y",
                "@deepseek-ai/dsh@0.1.5-rc.1",
                "--profile",
                "acp",
                "--dump-config",
                "--patch",
            ])
            .arg(&safety)
            .arg("--patch")
            .arg(&native)
            .env("npm_config_registry", "https://registry.npmjs.org")
            .env("DSH_TELEMETRY_DISABLED", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Harness config failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let config = String::from_utf8(output.stdout).unwrap();
        let filesystem = config
            .split("- id: skill-filesystem")
            .nth(1)
            .unwrap()
            .split("- id: ")
            .next()
            .unwrap();
        assert!(filesystem.contains("includeDefaultRoots: false"));
        assert!(filesystem.contains(root.to_string_lossy().as_ref()));
        let loader = config
            .split("- id: tool-skill")
            .nth(1)
            .unwrap()
            .split("- id: ")
            .next()
            .unwrap();
        assert!(!loader.contains("disabled: true"));
        for id in ["tool-bash", "tool-fs", "tool-web", "tool-subagent"] {
            let row = config
                .split(&format!("- id: {id}\n"))
                .nth(1)
                .unwrap()
                .split("- id: ")
                .next()
                .unwrap();
            assert!(row.contains("disabled: true"), "{id} unexpectedly enabled");
        }
    }
}
