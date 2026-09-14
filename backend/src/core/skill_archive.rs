use std::io::{Cursor, Read};
use std::path::Component;

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::error::BrainError;

const MAX_FILES: usize = 100;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSkillArchive {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub files: Vec<(String, String)>,
}

pub fn parse_skill_archive(bytes: &[u8]) -> Result<ImportedSkillArchive, BrainError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| BrainError::KnowledgeValidation(format!("Skill ZIP 无效: {error}")))?;
    if archive.len() > MAX_FILES {
        return Err(BrainError::KnowledgeValidation(format!(
            "Skill ZIP 文件数不能超过 {MAX_FILES}"
        )));
    }
    let mut files = Vec::new();
    let mut expanded = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            BrainError::KnowledgeValidation(format!("Skill ZIP 读取失败: {error}"))
        })?;
        if file.is_dir() {
            continue;
        }
        if file.size() > MAX_FILE_BYTES {
            return Err(BrainError::KnowledgeValidation(format!(
                "Skill 文件过大: {}",
                file.name()
            )));
        }
        expanded = expanded.saturating_add(file.size());
        if expanded > MAX_EXPANDED_BYTES {
            return Err(BrainError::KnowledgeValidation(
                "Skill ZIP 展开后不能超过 2 MB".to_string(),
            ));
        }
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(BrainError::KnowledgeValidation(
                "Skill ZIP 不允许包含符号链接".to_string(),
            ));
        }
        let raw_name = file.name().replace('\\', "/");
        let path = std::path::Path::new(&raw_name);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(BrainError::KnowledgeValidation(format!(
                "Skill ZIP 包含不安全路径: {raw_name}"
            )));
        }
        let relative_path = path.to_string_lossy().to_string();
        let lower = relative_path.to_ascii_lowercase();
        if lower.ends_with(".sh")
            || lower.ends_with(".py")
            || lower.ends_with(".js")
            || lower.ends_with(".exe")
            || lower.ends_with(".dll")
            || lower.ends_with(".dylib")
        {
            return Err(BrainError::KnowledgeValidation(format!(
                "首版仅支持指令型 Skill，不允许脚本文件: {relative_path}"
            )));
        }
        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|_| {
            BrainError::KnowledgeValidation(format!("首版仅支持 UTF-8 文本资源: {relative_path}"))
        })?;
        files.push((relative_path, content));
    }
    let skill_document = files
        .iter()
        .find(|(path, _)| path == "SKILL.md")
        .map(|(_, content)| content.as_str())
        .ok_or_else(|| {
            BrainError::KnowledgeValidation("Skill ZIP 根目录缺少 SKILL.md".to_string())
        })?;
    let metadata = frontmatter(skill_document);
    let name = metadata
        .iter()
        .find(|(key, _)| key == "name")
        .map(|(_, value)| value.trim().trim_matches(['\'', '"']).to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            BrainError::KnowledgeValidation("SKILL.md frontmatter 缺少 name".to_string())
        })?;
    let description = metadata
        .iter()
        .find(|(key, _)| key == "description")
        .map(|(_, value)| value.trim().trim_matches(['\'', '"']).to_string())
        .unwrap_or_default();
    let declared_slug = metadata
        .iter()
        .find(|(key, _)| key == "slug")
        .map(|(_, value)| value.trim().trim_matches(['\'', '"']))
        .unwrap_or("");
    let mut slug = slugify(if declared_slug.is_empty() {
        &name
    } else {
        declared_slug
    });
    if slug.is_empty() {
        let hash = hex::encode(Sha256::digest(skill_document.as_bytes()));
        slug = format!("imported-{}", &hash[..12]);
    }
    Ok(ImportedSkillArchive {
        slug,
        name,
        description,
        files,
    })
}

fn frontmatter(content: &str) -> Vec<(String, String)> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    lines
        .take_while(|line| line.trim() != "---")
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect()
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            slug.push(character.to_ascii_lowercase());
        } else {
            separator = true;
        }
    }
    slug.truncate(64);
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use super::*;

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (path, content) in files {
            writer
                .start_file(path, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn test_parse_skill_archive_accepts_instruction_package() {
        let bytes = archive(&[
            (
                "SKILL.md",
                "---\nname: Argument Mapper\ndescription: Build arguments\n---\nUse evidence.",
            ),
            ("references/template.md", "# Template"),
        ]);
        let parsed = parse_skill_archive(&bytes).unwrap();
        assert_eq!(parsed.slug, "argument-mapper");
        assert_eq!(parsed.files.len(), 2);
    }

    #[test]
    fn test_parse_skill_archive_rejects_traversal_and_scripts() {
        let traversal = archive(&[("../SKILL.md", "---\nname: Bad\n---")]);
        assert!(parse_skill_archive(&traversal).is_err());
        let script = archive(&[
            ("SKILL.md", "---\nname: Bad\n---"),
            ("scripts/run.py", "print('bad')"),
        ]);
        assert!(parse_skill_archive(&script).is_err());
    }
}
