use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    CancelNotification, ContentBlock, ContentChunk, InitializeRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, ToolCallStatus,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::util::MatchDispatch;
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent, ConnectionTo, SessionMessage};
use async_trait::async_trait;

use crate::error::BrainError;
use crate::models::book_wiki::{RuntimeHealth, RuntimeProfile};

pub const AGENT_RUNTIME_CANCELLED: &str = "OBSIDIANBRAIN_AGENT_RUNTIME_CANCELLED";

#[derive(Clone, Debug)]
pub struct AgentPromptRequest {
    pub command: String,
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub patch_paths: Vec<PathBuf>,
    pub credential_env: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentRuntimeEvent {
    TextDelta {
        delta: String,
    },
    Thinking,
    ToolStarted {
        tool_call_id: String,
        title: String,
        kind: String,
    },
    ToolFinished {
        tool_call_id: String,
        title: Option<String>,
        status: String,
    },
    UsageContext {
        used: u64,
        size: u64,
    },
}

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError>;

    async fn prompt_with_events(
        &self,
        request: AgentPromptRequest,
        _events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
        mut cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, BrainError> {
        tokio::select! {
            result = self.prompt(request) => result,
            changed = cancel.changed() => {
                if changed.is_ok() && *cancel.borrow() {
                    Err(BrainError::KnowledgeValidation("Agent 运行已取消".to_string()))
                } else {
                    Err(BrainError::Internal("Agent 取消通道意外关闭".to_string()))
                }
            }
        }
    }
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

    #[allow(unused_assignments)]
    async fn run_prompt(
        &self,
        request: AgentPromptRequest,
        events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, BrainError> {
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
        for patch_path in &request.patch_paths {
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
                        "DeepSeek Harness 请求额外权限，已按受控工具策略拒绝"
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
                        let mut cancel = cancel;
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
                        let mut output = String::new();
                        let mut cancel_channel_closed = false;
                        loop {
                            tokio::select! {
                                update = session.read_update() => {
                                    match update? {
                                        SessionMessage::SessionMessage(dispatch) => {
                                            MatchDispatch::new(dispatch)
                                                .if_notification(async |notification: SessionNotification| {
                                                    handle_session_update(
                                                        notification.update,
                                                        &mut output,
                                                        events.as_ref(),
                                                    );
                                                    Ok(())
                                                })
                                                .await
                                                .otherwise_ignore()?;
                                        }
                                        SessionMessage::StopReason(_) => break,
                                        _ => {}
                                    }
                                }
                                changed = cancel.changed(), if !cancel_channel_closed => {
                                    match changed {
                                        Ok(()) if *cancel.borrow() => {
                                            session.connection().send_notification(
                                                CancelNotification::new(session.session_id().clone()),
                                            )?;
                                            return Err(agent_client_protocol::util::internal_error(
                                                AGENT_RUNTIME_CANCELLED,
                                            ));
                                        }
                                        Ok(()) => {}
                                        Err(_) => cancel_channel_closed = true,
                                    }
                                }
                            }
                        }
                        Ok(output)
                    })
                    .await
            });

        let answer = tokio::time::timeout(self.timeout, operation)
            .await
            .map_err(|_| harness_error("DeepSeek Harness 在 180 秒内没有完成回答"))?
            .map_err(|error| {
                if error.to_string().contains(AGENT_RUNTIME_CANCELLED) {
                    BrainError::KnowledgeValidation("Agent 运行已取消".to_string())
                } else {
                    harness_error(acp_error_message(
                        &error.to_string(),
                        request.credential_env.as_deref(),
                    ))
                }
            })?;
        if answer.trim().is_empty() {
            return Err(harness_error("DeepSeek Harness 返回了空回答"));
        }
        Ok(answer)
    }
}

#[async_trait]
impl AgentRuntime for DeepSeekHarnessRuntime {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
        let (_cancel_guard, cancel) = tokio::sync::watch::channel(false);
        self.run_prompt(request, None, cancel).await
    }

    async fn prompt_with_events(
        &self,
        request: AgentPromptRequest,
        events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, BrainError> {
        self.run_prompt(request, events, cancel).await
    }
}

fn handle_session_update(
    update: SessionUpdate,
    output: &mut String,
    events: Option<&tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
) {
    match update {
        SessionUpdate::AgentMessageChunk(ContentChunk {
            content: ContentBlock::Text(text),
            ..
        }) => {
            output.push_str(&text.text);
            emit_event(events, AgentRuntimeEvent::TextDelta { delta: text.text });
        }
        SessionUpdate::AgentThoughtChunk(_) => {
            emit_event(events, AgentRuntimeEvent::Thinking);
        }
        SessionUpdate::ToolCall(tool) => {
            emit_event(
                events,
                AgentRuntimeEvent::ToolStarted {
                    tool_call_id: tool.tool_call_id.to_string(),
                    title: tool.title,
                    kind: format!("{:?}", tool.kind).to_ascii_lowercase(),
                },
            );
        }
        SessionUpdate::ToolCallUpdate(update) => {
            if let Some(status) = update.fields.status {
                if matches!(status, ToolCallStatus::Completed | ToolCallStatus::Failed) {
                    emit_event(
                        events,
                        AgentRuntimeEvent::ToolFinished {
                            tool_call_id: update.tool_call_id.to_string(),
                            title: update.fields.title,
                            status: format!("{status:?}").to_ascii_lowercase(),
                        },
                    );
                }
            }
        }
        SessionUpdate::UsageUpdate(usage) => {
            emit_event(
                events,
                AgentRuntimeEvent::UsageContext {
                    used: usage.used,
                    size: usage.size,
                },
            );
        }
        _ => {}
    }
}

fn emit_event(
    events: Option<&tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
    event: AgentRuntimeEvent,
) {
    if let Some(events) = events {
        let _ = events.send(event);
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

fn acp_error_message(detail: &str, credential_env: Option<&str>) -> String {
    let normalized = detail.to_ascii_lowercase();
    if normalized.contains("no api key")
        || normalized.contains("missing_credential")
        || normalized.contains("no credential")
    {
        return match credential_env {
            Some(environment) => format!(
                "DeepSeek Harness 未找到模型凭据 {environment}。请在启动 ObsidianBrain 前设置该环境变量；密钥不会保存到知识库数据库"
            ),
            None => "DeepSeek Harness 未找到当前模型供应商的 API Key。请在 Harness Web 的 Models 页面保存凭据，或为 Runtime 配置凭据环境变量"
                .to_string(),
        };
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
            provider_config: None,
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
    fn test_acp_error_message_uses_configured_credential_reference() {
        let message = acp_error_message(
            "turn failed: llm-pi-ai: no credential for provider route aliyun-bailian; its profile resolves CUSTOM_LLM_API_KEY, which is not set: internal data",
            Some("CUSTOM_LLM_API_KEY"),
        );

        assert!(message.contains("CUSTOM_LLM_API_KEY"));
        assert!(!message.contains("internal data"));
    }

    #[test]
    fn test_handle_session_update_emits_text_thought_and_usage_events() {
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let mut output = String::new();

        handle_session_update(
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                agent_client_protocol::schema::v1::TextContent::new("答案"),
            ))),
            &mut output,
            Some(&events),
        );
        handle_session_update(
            SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                agent_client_protocol::schema::v1::TextContent::new("不应落库的推理"),
            ))),
            &mut output,
            Some(&events),
        );
        handle_session_update(
            SessionUpdate::UsageUpdate(agent_client_protocol::schema::v1::UsageUpdate::new(
                128, 4096,
            )),
            &mut output,
            Some(&events),
        );

        assert_eq!(output, "答案");
        assert_eq!(
            received.try_recv().unwrap(),
            AgentRuntimeEvent::TextDelta {
                delta: "答案".to_string()
            }
        );
        assert_eq!(received.try_recv().unwrap(), AgentRuntimeEvent::Thinking);
        assert_eq!(
            received.try_recv().unwrap(),
            AgentRuntimeEvent::UsageContext {
                used: 128,
                size: 4096
            }
        );
        assert!(received.try_recv().is_err());
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
                patch_paths: Vec::new(),
                credential_env: None,
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
                patch_paths: Vec::new(),
                credential_env: None,
            })
            .await
            .expect("real ACP response");

        assert!(answer.contains("连接成功"));
    }
}
