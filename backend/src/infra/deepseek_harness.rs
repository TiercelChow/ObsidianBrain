use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    InitializeRequest, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SetSessionConfigOptionRequest,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent, ConnectionTo};
use async_trait::async_trait;

use crate::error::BrainError;
use crate::models::book_wiki::{RuntimeHealth, RuntimeProfile};

#[derive(Clone, Debug)]
pub struct AgentPromptRequest {
    pub command: String,
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub patch_path: Option<PathBuf>,
}

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError>;
}

#[derive(Clone, Debug)]
pub struct DeepSeekHarnessRuntime {
    timeout: Duration,
}

impl Default for DeepSeekHarnessRuntime {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(180),
        }
    }
}

impl DeepSeekHarnessRuntime {
    #[cfg(test)]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl AgentRuntime for DeepSeekHarnessRuntime {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
        if request.command.trim().is_empty() {
            return Err(harness_error("ACP 启动命令为空"));
        }
        if !request.cwd.is_absolute() || !request.cwd.is_dir() {
            return Err(harness_error("ACP 会话目录必须是已存在的绝对目录"));
        }

        let mut command_parts = shell_words::split(&request.command)
            .map_err(|error| harness_error(format!("无法解析 ACP 启动命令: {error}")))?;
        if command_parts.is_empty() {
            return Err(harness_error("ACP 启动命令为空"));
        }
        let command = command_parts.remove(0);
        if let Some(patch_path) = &request.patch_path {
            if !patch_path.is_absolute() || !patch_path.is_file() {
                return Err(harness_error("Harness Patch 必须是已存在的绝对文件"));
            }
            command_parts.push("--patch".to_string());
            command_parts.push(patch_path.to_string_lossy().to_string());
        }
        let agent = AcpAgent::new(AcpAgentConfig::new(command).args(command_parts));
        let cwd = request.cwd;
        let model = request.model;
        let prompt = request.prompt;

        let operation = agent_client_protocol::Client
            .builder()
            .name("ObsidianBrain")
            .on_receive_request(
                async move |request: RequestPermissionRequest, responder, _connection| {
                    tracing::warn!(
                        session_id = %request.session_id,
                        "DeepSeek Harness 请求额外权限，已按只读策略拒绝"
                    );
                    responder.respond(RequestPermissionResponse::new(
                        RequestPermissionOutcome::Cancelled,
                    ))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
                connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;

                connection
                    .build_session(&cwd)
                    .block_task()
                    .run_until(async move |mut session| {
                        if !model.trim().is_empty() {
                            session
                                .connection()
                                .send_request(SetSessionConfigOptionRequest::new(
                                    session.session_id().clone(),
                                    "model",
                                    model.as_str(),
                                ))
                                .block_task()
                                .await?;
                        }
                        session.send_prompt(prompt)?;
                        session.read_to_string().await
                    })
                    .await
            });

        let answer = tokio::time::timeout(self.timeout, operation)
            .await
            .map_err(|_| harness_error("DeepSeek Harness 在 180 秒内没有完成回答"))?
            .map_err(|error| harness_error(acp_error_message(&error.to_string())))?;
        if answer.trim().is_empty() {
            return Err(harness_error("DeepSeek Harness 返回了空回答"));
        }
        Ok(answer)
    }
}

pub fn inspect_runtime_profiles(profiles: Vec<RuntimeProfile>) -> Vec<RuntimeHealth> {
    profiles.into_iter().map(inspect_runtime_profile).collect()
}

fn inspect_runtime_profile(profile: RuntimeProfile) -> RuntimeHealth {
    if !profile.enabled {
        return RuntimeHealth {
            profile,
            available: false,
            version: None,
            message: "运行时已停用".to_string(),
        };
    }

    let parts = match shell_words::split(&profile.executable) {
        Ok(parts) if !parts.is_empty() => parts,
        Ok(_) => {
            return RuntimeHealth {
                profile,
                available: false,
                version: None,
                message: "ACP 启动命令为空".to_string(),
            };
        }
        Err(error) => {
            return RuntimeHealth {
                profile,
                available: false,
                version: None,
                message: format!("ACP 启动命令无法解析: {error}"),
            };
        }
    };

    let is_npx = std::path::Path::new(&parts[0])
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("npx"));
    let mut command = Command::new(&parts[0]);
    if is_npx {
        command.arg("--version");
    } else {
        command.args(&parts[1..]).arg("--version");
    }

    match command.output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let version = stdout
                .lines()
                .chain(stderr.lines())
                .find(|line| !line.trim().is_empty())
                .map(|line| {
                    if is_npx {
                        format!("npx {}", line.trim())
                    } else {
                        line.trim().to_string()
                    }
                });
            RuntimeHealth {
                profile,
                available: true,
                version,
                message: "本地启动器可用；尚未验证 Harness 会话、网络与模型凭据".to_string(),
            }
        }
        Ok(output) => RuntimeHealth {
            profile,
            available: false,
            version: None,
            message: format!("ACP 运行时返回状态 {}", output.status),
        },
        Err(error) => RuntimeHealth {
            profile,
            available: false,
            version: None,
            message: format!("未找到 ACP 启动程序: {error}"),
        },
    }
}

fn harness_error(detail: impl Into<String>) -> BrainError {
    BrainError::LlmApiError {
        provider: "deepseek_harness".to_string(),
        detail: detail.into(),
    }
}

fn acp_error_message(detail: &str) -> String {
    if detail.contains("no API key") || detail.contains("MISSING_CREDENTIAL") {
        return "DeepSeek Harness 未配置 DEEPSEEK_API_KEY。请在 Harness Web 的 Models 页面保存密钥，或在启动 ObsidianBrain 前导出该环境变量"
            .to_string();
    }
    format!("ACP 调用失败: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(executable: &str, enabled: bool) -> RuntimeProfile {
        RuntimeProfile {
            id: "runtime-test".to_string(),
            name: "Test".to_string(),
            runtime: "deepseek_harness".to_string(),
            executable: executable.to_string(),
            model: String::new(),
            enabled,
            revision: 1,
            updated_at: String::new(),
        }
    }

    #[test]
    fn test_inspect_runtime_profiles_reports_missing_executable() {
        let health = inspect_runtime_profiles(vec![profile(
            "/path/that/does/not/exist/deepseek-harness --profile acp",
            true,
        )]);

        assert_eq!(health.len(), 1);
        assert!(!health[0].available);
        assert!(health[0].message.contains("未找到"));
    }

    #[test]
    fn test_inspect_runtime_profiles_skips_disabled_profile() {
        let health = inspect_runtime_profiles(vec![profile("missing-command", false)]);

        assert!(!health[0].available);
        assert_eq!(health[0].message, "运行时已停用");
    }

    #[test]
    fn test_acp_error_message_simplifies_missing_credentials() {
        let message = acp_error_message(
            "turn failed: no API key for provider route deepseek-official: internal data",
        );

        assert!(message.contains("DEEPSEEK_API_KEY"));
        assert!(!message.contains("internal data"));
    }

    #[tokio::test]
    async fn test_prompt_rejects_relative_session_directory() {
        let runtime = DeepSeekHarnessRuntime::with_timeout(Duration::from_millis(10));
        let error = runtime
            .prompt(AgentPromptRequest {
                command: "npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp".to_string(),
                model: String::new(),
                cwd: PathBuf::from("relative"),
                prompt: "test".to_string(),
                patch_path: None,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("绝对目录"));
    }

    #[tokio::test]
    #[ignore = "requires local DeepSeek Harness credentials and network access"]
    async fn test_real_deepseek_harness_acp_returns_text() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let answer = DeepSeekHarnessRuntime::default()
            .prompt(AgentPromptRequest {
                command: "npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp".to_string(),
                model: String::new(),
                cwd: workspace.path().to_path_buf(),
                prompt: "只回答：连接成功".to_string(),
                patch_path: None,
            })
            .await
            .expect("real ACP response");

        assert!(answer.contains("连接成功"));
    }
}
