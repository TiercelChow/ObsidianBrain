//! VSCode 集成

use std::path::Path;

use crate::models::repo::VscodeResult;

/// VSCode 打开器
pub struct VscodeOpener;

impl VscodeOpener {
    /// 生成 vscode:// URI
    pub fn make_uri(repo_path: &Path) -> String {
        format!("vscode://file{}", repo_path.display())
    }

    /// 打开仓库（返回 URI，实际打开是 best-effort）
    pub fn open(repo_name: &str, repo_path: &Path) -> VscodeResult {
        let uri = Self::make_uri(repo_path);

        // 尝试通过系统命令打开
        #[cfg(target_os = "macos")]
        let opened = std::process::Command::new("open")
            .arg(&uri)
            .spawn()
            .is_ok();

        #[cfg(target_os = "linux")]
        let opened = std::process::Command::new("xdg-open")
            .arg(&uri)
            .spawn()
            .is_ok();

        #[cfg(target_os = "windows")]
        let opened = std::process::Command::new("cmd")
            .args(["/C", "start", &uri])
            .spawn()
            .is_ok();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        let opened = false;

        VscodeResult {
            repo_name: repo_name.to_string(),
            vscode_uri: uri,
            opened,
            message: if opened {
                "VSCode 已打开".to_string()
            } else {
                "请手动打开 VSCode".to_string()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_make_uri() {
        let path = PathBuf::from("/Users/test/project");
        let uri = VscodeOpener::make_uri(&path);
        assert_eq!(uri, "vscode://file/Users/test/project");
    }
}
