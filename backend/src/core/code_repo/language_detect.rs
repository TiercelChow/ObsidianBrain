//! 编程语言检测器

use std::collections::HashMap;
use std::path::Path;

/// 语言检测器
pub struct LanguageDetector;

impl LanguageDetector {
    /// 检测仓库中的语言构成
    pub fn detect(root: &Path, exclude_dirs: &[String], max_files: usize) -> HashMap<String, f32> {
        let mut lang_lines: HashMap<String, u64> = HashMap::new();
        let mut total_lines: u64 = 0;
        let mut file_count = 0;

        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                if file_count >= max_files {
                    break;
                }
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();

                // 跳过排除目录
                if path.is_dir() {
                    if exclude_dirs.iter().any(|d| d.as_str() == name.as_ref()) {
                        continue;
                    }
                    // 递归子目录
                    let sub_result = Self::detect(&path, exclude_dirs, max_files - file_count);
                    for (lang, lines) in sub_result {
                        let lines_u64 = (lines * 1000.0) as u64;
                        *lang_lines.entry(lang).or_insert(0) += lines_u64;
                        total_lines += lines_u64;
                    }
                    continue;
                }

                // 处理文件
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if let Some(lang) = Self::ext_to_language(&ext_str) {
                        let lines = Self::count_lines(&path) as u64;
                        if lines > 0 {
                            *lang_lines.entry(lang.to_string()).or_insert(0) += lines;
                            total_lines += lines;
                            file_count += 1;
                        }
                    }
                }
            }
        }

        // 计算比例
        if total_lines == 0 {
            return HashMap::new();
        }

        let mut stats: HashMap<String, f32> = HashMap::new();
        for (lang, lines) in &lang_lines {
            let ratio = (*lines as f32) / (total_lines as f32);
            if ratio >= 0.01 {
                stats.insert(lang.clone(), (ratio * 100.0).round() / 100.0);
            }
        }
        stats
    }

    /// 扩展名到语言的映射
    fn ext_to_language(ext: &str) -> Option<&'static str> {
        match ext {
            "rs" => Some("Rust"),
            "py" | "pyi" | "pyx" => Some("Python"),
            "js" | "mjs" | "cjs" | "jsx" => Some("JavaScript"),
            "ts" | "mts" | "cts" | "tsx" => Some("TypeScript"),
            "java" => Some("Java"),
            "kt" | "kts" => Some("Kotlin"),
            "c" | "h" => Some("C"),
            "cpp" | "cc" | "cxx" | "hpp" => Some("C++"),
            "go" => Some("Go"),
            "rb" => Some("Ruby"),
            "php" => Some("PHP"),
            "sh" | "bash" | "zsh" => Some("Shell"),
            "html" | "htm" => Some("HTML"),
            "css" | "scss" | "sass" | "less" => Some("CSS"),
            "vue" => Some("Vue"),
            "svelte" => Some("Svelte"),
            "toml" => Some("TOML"),
            "yaml" | "yml" => Some("YAML"),
            "json" => Some("JSON"),
            "md" | "mdx" => Some("Markdown"),
            "sql" => Some("SQL"),
            "swift" => Some("Swift"),
            "dart" => Some("Dart"),
            "lua" => Some("Lua"),
            "zig" => Some("Zig"),
            "ex" | "exs" => Some("Elixir"),
            "hs" => Some("Haskell"),
            "proto" => Some("Protobuf"),
            "tf" => Some("Terraform"),
            _ => None,
        }
    }

    /// 统计文件行数
    fn count_lines(path: &Path) -> u32 {
        std::fs::read_to_string(path)
            .map(|content| {
                content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count() as u32
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_rust_project() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn test() {}\n").unwrap();

        let stats = LanguageDetector::detect(dir.path(), &[], 100);
        assert!(stats.contains_key("Rust"));
        // Rust should be 100%
        assert_eq!(stats["Rust"], 1.0);
    }

    #[test]
    fn test_detect_mixed_languages() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.path().join("index.ts"), "console.log('hi');\n").unwrap();

        let stats = LanguageDetector::detect(dir.path(), &[], 100);
        assert!(stats.contains_key("Rust"));
        assert!(stats.contains_key("TypeScript"));
    }

    #[test]
    fn test_exclude_dirs() {
        let dir = TempDir::new().unwrap();
        let node_modules = dir.path().join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        fs::write(node_modules.join("index.js"), "module.exports = {};\n").unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

        let stats = LanguageDetector::detect(dir.path(), &["node_modules".to_string()], 100);
        assert!(!stats.contains_key("JavaScript"));
        assert!(stats.contains_key("Rust"));
    }

    #[test]
    fn test_ext_to_language() {
        assert_eq!(LanguageDetector::ext_to_language("rs"), Some("Rust"));
        assert_eq!(LanguageDetector::ext_to_language("py"), Some("Python"));
        assert_eq!(LanguageDetector::ext_to_language("xyz"), None);
    }
}
