use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    CancelNotification, ContentBlock, ContentChunk, InitializeRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SessionNotification, SessionUpdate,
    SetSessionConfigOptionRequest, StopReason, ToolCallStatus,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::util::MatchDispatch;
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent, ConnectionTo, SessionMessage};
use async_trait::async_trait;

use crate::error::BrainError;
use crate::models::book_wiki::{RuntimeHealth, RuntimeProfile};

pub const AGENT_RUNTIME_CANCELLED: &str = "OBSIDIANBRAIN_AGENT_RUNTIME_CANCELLED";
const AGENT_RUNTIME_IDLE_TIMEOUT: &str = "OBSIDIANBRAIN_AGENT_RUNTIME_IDLE_TIMEOUT";

#[derive(Clone)]
pub struct AgentPromptRequest {
    pub command: String,
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub patch_paths: Vec<PathBuf>,
    pub credential_env: Option<String>,
    pub credential_value: Option<String>,
    pub timeout: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentRuntimeEvent {
    Phase {
        phase: String,
        message: String,
    },
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
    /// ACP context occupancy is not billable token usage. Optional cost is a
    /// separately reported cumulative session value, never a per-update delta.
    UsageCost {
        amount: f64,
        currency: String,
    },
    /// A nonempty answer is complete only after a successful end_turn.
    Completed {
        stop_reason: String,
        complete: bool,
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
    idle_timeout: Duration,
}

impl Default for DeepSeekHarnessRuntime {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(180),
            idle_timeout: Duration::from_secs(600),
        }
    }
}

impl DeepSeekHarnessRuntime {
    #[cfg(test)]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Self::default()
        }
    }

    #[cfg(test)]
    fn with_idle_timeout(idle_timeout: Duration) -> Self {
        Self {
            idle_timeout,
            ..Self::default()
        }
    }

    #[allow(unused_assignments)]
    async fn run_prompt(
        &self,
        request: AgentPromptRequest,
        events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, BrainError> {
        if *cancel.borrow() {
            emit_completion(events.as_ref(), "cancelled", false);
            return Err(BrainError::KnowledgeValidation(
                "Agent 运行已取消".to_string(),
            ));
        }
        if request.command.trim().is_empty() {
            return Err(harness_error("ACP 启动命令为空"));
        }
        if !request.cwd.is_absolute() || !request.cwd.is_dir() {
            return Err(harness_error("ACP 会话目录必须是已存在的绝对目录"));
        }

        let request_timeout = request.timeout.unwrap_or(self.timeout);
        let idle_timeout = self.idle_timeout.min(request_timeout);
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
        emit_event(
            events.as_ref(),
            AgentRuntimeEvent::Phase {
                phase: "launching".to_string(),
                message: "正在启动 DeepSeek Harness 进程".to_string(),
            },
        );
        let mut agent_config = AcpAgentConfig::new(command).args(command_parts);
        if let (Some(environment), Some(secret)) = (
            request.credential_env.as_ref(),
            request.credential_value.as_ref(),
        ) {
            agent_config = agent_config.env(environment, secret);
        }
        let agent = AcpAgent::new(agent_config);
        let cwd = request.cwd;
        let model = request.model;
        let prompt = request.prompt;
        let operation_events = events.clone();
        let completion_events = events.clone();

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
                emit_event(
                    operation_events.as_ref(),
                    AgentRuntimeEvent::Phase {
                        phase: "connected".to_string(),
                        message: "Harness ACP 连接已建立".to_string(),
                    },
                );

                connection
                    .build_session(&cwd)
                    .block_task()
                    .run_until(async move |mut session| {
                        let mut cancel = cancel;
                        emit_event(
                            operation_events.as_ref(),
                            AgentRuntimeEvent::Phase {
                                phase: "session_ready".to_string(),
                                message: "ACP 会话已就绪".to_string(),
                            },
                        );
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
                        emit_event(
                            operation_events.as_ref(),
                            AgentRuntimeEvent::Phase {
                                phase: "request_sent".to_string(),
                                message: "模型请求已提交，正在等待返回".to_string(),
                            },
                        );
                        let mut output = String::new();
                        let mut cancel_channel_closed = false;
                        let idle_timer = tokio::time::sleep(idle_timeout);
                        tokio::pin!(idle_timer);
                        let stop_reason = loop {
                            tokio::select! {
                                update = session.read_update() => {
                                    let update = update?;
                                    idle_timer.as_mut().reset(tokio::time::Instant::now() + idle_timeout);
                                    match update {
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
                                        SessionMessage::StopReason(reason) => break reason,
                                        _ => {}
                                    }
                                }
                                _ = &mut idle_timer => {
                                    return Err(agent_client_protocol::util::internal_error(
                                        AGENT_RUNTIME_IDLE_TIMEOUT,
                                    ));
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
                        };
                        emit_event(
                            operation_events.as_ref(),
                            AgentRuntimeEvent::Phase {
                                phase: "stopped".to_string(),
                                message: format!(
                                    "ACP 会话结束：{}",
                                    stop_reason_code(stop_reason)
                                ),
                            },
                        );
                        Ok((output, stop_reason))
                    })
                    .await
            });

        let (answer, stop_reason) = tokio::time::timeout(request_timeout, operation)
            .await
            .map_err(|_| {
                emit_completion(completion_events.as_ref(), "timeout", false);
                harness_error(format!(
                    "DeepSeek Harness 在 {}内没有完成回答",
                    duration_label(request_timeout)
                ))
            })?
            .map_err(|error| {
                if error.to_string().contains(AGENT_RUNTIME_CANCELLED) {
                    emit_completion(completion_events.as_ref(), "cancelled", false);
                    BrainError::KnowledgeValidation("Agent 运行已取消".to_string())
                } else if error.to_string().contains(AGENT_RUNTIME_IDLE_TIMEOUT) {
                    emit_completion(completion_events.as_ref(), "idle_timeout", false);
                    harness_error(format!(
                        "(idle_timeout) DeepSeek Harness 连续 {}没有收到进展，已停止本次运行；部分输出保留",
                        duration_label(idle_timeout)
                    ))
                } else {
                    emit_completion(completion_events.as_ref(), "runtime_error", false);
                    harness_error(acp_error_message(
                        &error.to_string(),
                        request.credential_env.as_deref(),
                    ))
                }
            })?;
        finish_answer(answer, stop_reason, completion_events.as_ref())
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
            if let Some(cost) = usage.cost {
                if cost.amount.is_finite() && cost.amount >= 0.0 && !cost.currency.trim().is_empty()
                {
                    emit_event(
                        events,
                        AgentRuntimeEvent::UsageCost {
                            amount: cost.amount,
                            currency: cost.currency,
                        },
                    );
                }
            }
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

fn duration_label(duration: Duration) -> String {
    if duration.subsec_nanos() == 0 {
        format!("{} 秒", duration.as_secs())
    } else {
        format!("{} 毫秒", duration.as_millis())
    }
}

fn stop_reason_code(reason: StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::MaxTurnRequests => "max_turn_requests",
        StopReason::Refusal => "refusal",
        StopReason::Cancelled => "cancelled",
        _ => "unknown",
    }
}

fn emit_completion(
    events: Option<&tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
    stop_reason: &str,
    complete: bool,
) {
    emit_event(
        events,
        AgentRuntimeEvent::Completed {
            stop_reason: stop_reason.to_string(),
            complete,
        },
    );
}

/// Never convert exhausted or refused turns into successful business results.
/// Their already emitted TextDelta events remain available for partial display.
fn finish_answer(
    answer: String,
    reason: StopReason,
    events: Option<&tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
) -> Result<String, BrainError> {
    let empty = answer.trim().is_empty();
    let complete = !empty && matches!(reason, StopReason::EndTurn);
    emit_completion(events, stop_reason_code(reason), complete);
    if complete {
        return Ok(answer);
    }
    if empty {
        return Err(empty_answer_error(reason));
    }
    match reason {
        StopReason::Cancelled => Err(BrainError::KnowledgeValidation("Agent 运行已取消".to_string())),
        StopReason::MaxTokens => Err(harness_error(
            "DeepSeek Harness 达到输出 token 上限，回答未完成；已生成正文保留为部分结果 (stop_reason=max_tokens)",
        )),
        StopReason::MaxTurnRequests => Err(harness_error(
            "DeepSeek Harness 达到请求轮次上限，回答未完成；已生成正文保留为部分结果 (stop_reason=max_turn_requests)",
        )),
        StopReason::Refusal => Err(empty_answer_error(reason)),
        _ => Err(harness_error(format!(
            "DeepSeek Harness 未确认完整结束，回答未完成 (stop_reason={})",
            stop_reason_code(reason)
        ))),
    }
}

fn empty_answer_error(reason: StopReason) -> BrainError {
    let detail = match reason {
        StopReason::MaxTokens => "DeepSeek Harness 达到输出 token 上限且未返回正文",
        StopReason::MaxTurnRequests => "DeepSeek Harness 达到请求轮次上限且未返回正文",
        StopReason::Refusal => "DeepSeek Harness 拒绝回答",
        StopReason::Cancelled => {
            return BrainError::KnowledgeValidation("Agent 运行已取消".to_string())
        }
        _ => "DeepSeek Harness 返回了空回答",
    };
    harness_error(format!(
        "{detail} (stop_reason={})",
        stop_reason_code(reason)
    ))
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
                "DeepSeek Harness 未找到模型凭据 {environment}。请在 Wiki 配置页补充 API Key，或检查所选环境变量"
            ),
            None => "DeepSeek Harness 未找到当前模型供应商的 API Key。请在 Wiki 配置页选择供应商并保存凭据"
                .to_string(),
        };
    }
    format!("ACP 调用失败: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_label_reports_the_effective_request_deadline() {
        assert_eq!(duration_label(Duration::from_secs(600)), "600 秒");
        assert_eq!(duration_label(Duration::from_millis(25)), "25 毫秒");
    }

    fn profile(executable: &str, enabled: bool) -> RuntimeProfile {
        RuntimeProfile {
            id: "runtime-test".to_string(),
            name: "Test".to_string(),
            runtime: "deepseek_harness".to_string(),
            executable: executable.to_string(),
            model: String::new(),
            provider_id: None,
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

    #[test]
    fn test_empty_answer_reports_acp_stop_reason() {
        let exhausted =
            empty_answer_error(agent_client_protocol::schema::v1::StopReason::MaxTokens);
        assert!(exhausted.to_string().contains("输出 token 上限"));
        assert!(exhausted.to_string().contains("max_tokens"));

        let ended = empty_answer_error(agent_client_protocol::schema::v1::StopReason::EndTurn);
        assert!(ended.to_string().contains("空回答"));
        assert!(ended.to_string().contains("end_turn"));

        let refused = empty_answer_error(agent_client_protocol::schema::v1::StopReason::Refusal);
        assert!(refused.to_string().contains("拒绝回答"));
    }

    #[test]
    fn test_finish_answer_reports_a_nonempty_normal_completion() {
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();

        let answer =
            finish_answer("完整答案".to_string(), StopReason::EndTurn, Some(&events)).unwrap();

        assert_eq!(answer, "完整答案");
        assert_eq!(
            received.try_recv().unwrap(),
            AgentRuntimeEvent::Completed {
                stop_reason: "end_turn".to_string(),
                complete: true,
            }
        );
    }

    #[test]
    fn test_finish_answer_rejects_nonempty_truncation_without_losing_streamed_text() {
        for reason in [StopReason::MaxTokens, StopReason::MaxTurnRequests] {
            let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
            let mut output = String::new();
            handle_session_update(
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    agent_client_protocol::schema::v1::TextContent::new("已生成但尚未完成"),
                ))),
                &mut output,
                Some(&events),
            );

            let error = finish_answer(output, reason, Some(&events)).unwrap_err();

            assert!(error.to_string().contains(stop_reason_code(reason)));
            assert!(error.to_string().contains("未完成"));
            assert_eq!(
                received.try_recv().unwrap(),
                AgentRuntimeEvent::TextDelta {
                    delta: "已生成但尚未完成".to_string(),
                }
            );
            assert_eq!(
                received.try_recv().unwrap(),
                AgentRuntimeEvent::Completed {
                    stop_reason: stop_reason_code(reason).to_string(),
                    complete: false,
                }
            );
        }
    }

    #[test]
    fn test_finish_answer_rejects_empty_refused_and_cancelled_answers() {
        for (answer, reason) in [
            (" \n", StopReason::EndTurn),
            ("部分拒绝文本", StopReason::Refusal),
            ("部分取消文本", StopReason::Cancelled),
        ] {
            let (events, mut received) = tokio::sync::mpsc::unbounded_channel();

            assert!(finish_answer(answer.to_string(), reason, Some(&events)).is_err());
            assert_eq!(
                received.try_recv().unwrap(),
                AgentRuntimeEvent::Completed {
                    stop_reason: stop_reason_code(reason).to_string(),
                    complete: false,
                }
            );
        }
    }

    #[test]
    fn test_usage_context_remains_context_only_and_preserves_reported_cost() {
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let mut output = String::new();
        handle_session_update(
            SessionUpdate::UsageUpdate(
                agent_client_protocol::schema::v1::UsageUpdate::new(128, 4096)
                    .cost(agent_client_protocol::schema::v1::Cost::new(0.045, "USD")),
            ),
            &mut output,
            Some(&events),
        );

        assert_eq!(
            received.try_recv().unwrap(),
            AgentRuntimeEvent::UsageContext {
                used: 128,
                size: 4096
            }
        );
        assert_eq!(
            received.try_recv().unwrap(),
            AgentRuntimeEvent::UsageCost {
                amount: 0.045,
                currency: "USD".to_string()
            }
        );
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn test_usage_update_does_not_accept_invalid_cost() {
        for amount in [-1.0, f64::NAN, f64::INFINITY] {
            let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
            handle_session_update(
                SessionUpdate::UsageUpdate(
                    agent_client_protocol::schema::v1::UsageUpdate::new(128, 4096)
                        .cost(agent_client_protocol::schema::v1::Cost::new(amount, "USD")),
                ),
                &mut String::new(),
                Some(&events),
            );
            assert!(matches!(
                received.try_recv().unwrap(),
                AgentRuntimeEvent::UsageContext { .. }
            ));
            assert!(received.try_recv().is_err());
        }
    }

    // Exercise the real ACP transport without network, provider credentials or
    // Harness installation. POSIX sh is sufficient for this test-only agent.
    #[cfg(unix)]
    fn fake_acp_request(
        reason: &str,
        answer: &str,
        wait_for_cancel: bool,
    ) -> (tempfile::TempDir, AgentPromptRequest) {
        let workspace = tempfile::tempdir().unwrap();
        let script_path = workspace.path().join("fake-acp.sh");
        let notification = serde_json::json!({
            "jsonrpc": "2.0", "method": "session/update",
            "params": {"sessionId": "fixture-session", "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": answer}
            }}
        })
        .to_string();
        let settlement = if wait_for_cancel {
            "prompt_id=\"$request_id\"".to_string()
        } else {
            format!(
                "printf '{{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{{\"stopReason\":\"{reason}\"}}}}\\n' \"$request_id\""
            )
        };
        let script = format!(
            r#"while IFS= read -r request; do
    request_id=$(printf '%s\n' "$request" | sed -n 's/.*"id":\([^,}}]*\).*/\1/p')
    case "$request" in
        *'"method":"initialize"'*)
            printf '{{"jsonrpc":"2.0","id":%s,"result":{{"protocolVersion":1,"agentCapabilities":{{}}}}}}\n' "$request_id" ;;
        *'"method":"session/new"'*)
            printf '{{"jsonrpc":"2.0","id":%s,"result":{{"sessionId":"fixture-session"}}}}\n' "$request_id" ;;
        *'"method":"session/prompt"'*)
            printf '%s\n' {notification}
            {settlement} ;;
        *'"method":"session/cancel"'*)
            printf '{{"jsonrpc":"2.0","id":%s,"result":{{"stopReason":"cancelled"}}}}\n' "$prompt_id" ;;
    esac
done
"#,
            notification = shell_words::quote(&notification),
        );
        std::fs::write(&script_path, script).unwrap();
        let request = AgentPromptRequest {
            command: format!(
                "/bin/sh {}",
                shell_words::quote(&script_path.to_string_lossy())
            ),
            model: String::new(),
            cwd: workspace.path().to_path_buf(),
            prompt: "Transport fixture".to_string(),
            patch_paths: Vec::new(),
            credential_env: None,
            credential_value: None,
            timeout: Some(Duration::from_secs(3)),
        };
        (workspace, request)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_acp_transport_preserves_partial_output_and_stop_reason() {
        for (reason, answer, expected_complete) in [
            ("end_turn", "完整答案", true),
            ("end_turn", "", false),
            ("max_tokens", "输出一半", false),
            ("max_turn_requests", "工具执行一半", false),
            ("refusal", "拒绝回答", false),
            ("cancelled", "取消前输出", false),
        ] {
            let (_workspace, request) = fake_acp_request(reason, answer, false);
            let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
            let (_guard, cancel) = tokio::sync::watch::channel(false);
            let result = DeepSeekHarnessRuntime::default()
                .prompt_with_events(request, Some(events), cancel)
                .await;
            assert_eq!(result.is_ok(), expected_complete, "{reason}: {result:?}");
            let mut streamed = String::new();
            let mut completion = None;
            while let Ok(event) = received.try_recv() {
                match event {
                    AgentRuntimeEvent::TextDelta { delta } => streamed.push_str(&delta),
                    AgentRuntimeEvent::Completed {
                        stop_reason,
                        complete,
                    } => {
                        assert!(completion.is_none(), "duplicate completion");
                        completion = Some((stop_reason, complete));
                    }
                    _ => {}
                }
            }
            assert_eq!(streamed, answer);
            assert_eq!(completion, Some((reason.to_string(), expected_complete)));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_acp_transport_cancellation_preserves_streamed_partial_answer() {
        let (_workspace, request) = fake_acp_request("cancelled", "取消前输出", true);
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let (cancel_sender, cancel) = tokio::sync::watch::channel(false);
        let runtime = DeepSeekHarnessRuntime::default();
        let operation = runtime.prompt_with_events(request, Some(events), cancel);
        tokio::pin!(operation);
        let mut streamed = String::new();
        let mut completion = None;
        let result = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                tokio::select! {
                    result = &mut operation => break result,
                    event = received.recv() => {
                        match event {
                            Some(AgentRuntimeEvent::TextDelta { delta }) => {
                                streamed.push_str(&delta);
                                cancel_sender.send(true).unwrap();
                            }
                            Some(AgentRuntimeEvent::Completed { stop_reason, complete }) => {
                                completion = Some((stop_reason, complete));
                            }
                            _ => {}
                        }
                    }
                }
            }
        })
        .await
        .unwrap();
        assert!(result.unwrap_err().to_string().contains("取消"));
        while let Ok(event) = received.try_recv() {
            if let AgentRuntimeEvent::Completed {
                stop_reason,
                complete,
            } = event
            {
                assert!(completion.is_none(), "duplicate completion");
                completion = Some((stop_reason, complete));
            }
        }
        assert_eq!(streamed, "取消前输出");
        assert_eq!(completion, Some(("cancelled".to_string(), false)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_acp_transport_timeout_reports_incomplete_not_success() {
        let (_workspace, mut request) = fake_acp_request("end_turn", "未完成正文", true);
        request.timeout = Some(Duration::from_millis(250));
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let (_guard, cancel) = tokio::sync::watch::channel(false);
        let result = DeepSeekHarnessRuntime::default()
            .prompt_with_events(request, Some(events), cancel)
            .await;
        assert!(result.unwrap_err().to_string().contains("250 毫秒"));
        let mut streamed = String::new();
        let mut completion = None;
        while let Ok(event) = received.try_recv() {
            match event {
                AgentRuntimeEvent::TextDelta { delta } => streamed.push_str(&delta),
                AgentRuntimeEvent::Completed {
                    stop_reason,
                    complete,
                } => {
                    completion = Some((stop_reason, complete));
                }
                _ => {}
            }
        }
        assert_eq!(streamed, "未完成正文");
        assert_eq!(completion, Some(("timeout".to_string(), false)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_acp_transport_stalled_after_partial_output_reports_idle_timeout() {
        let (_workspace, mut request) = fake_acp_request("end_turn", "已有部分正文", true);
        request.timeout = Some(Duration::from_secs(2));
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let (_guard, cancel) = tokio::sync::watch::channel(false);
        let result = DeepSeekHarnessRuntime::with_idle_timeout(Duration::from_millis(150))
            .prompt_with_events(request, Some(events), cancel)
            .await;
        assert!(result.unwrap_err().to_string().contains("idle_timeout"));
        let mut streamed = String::new();
        let mut completion = None;
        while let Ok(event) = received.try_recv() {
            match event {
                AgentRuntimeEvent::TextDelta { delta } => streamed.push_str(&delta),
                AgentRuntimeEvent::Completed {
                    stop_reason,
                    complete,
                } => completion = Some((stop_reason, complete)),
                _ => {}
            }
        }
        assert_eq!(streamed, "已有部分正文");
        assert_eq!(completion, Some(("idle_timeout".to_string(), false)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_acp_transport_progress_resets_idle_deadline() {
        let (workspace, mut request) = fake_acp_request("end_turn", "unused", false);
        let update = |text: &str| {
            serde_json::json!({
                "jsonrpc":"2.0","method":"session/update",
                "params":{"sessionId":"fixture-session","update":{
                    "sessionUpdate":"agent_message_chunk","content":{"type":"text","text":text}
                }}
            })
            .to_string()
        };
        let script = format!(
            r#"while IFS= read -r request; do
    request_id=$(printf '%s\n' "$request" | sed -n 's/.*"id":\([^,}}]*\).*/\1/p')
    case "$request" in
        *'"method":"initialize"'*)
            printf '{{"jsonrpc":"2.0","id":%s,"result":{{"protocolVersion":1,"agentCapabilities":{{}}}}}}\n' "$request_id" ;;
        *'"method":"session/new"'*)
            printf '{{"jsonrpc":"2.0","id":%s,"result":{{"sessionId":"fixture-session"}}}}\n' "$request_id" ;;
        *'"method":"session/prompt"'*)
            printf '%s\n' {first}
            sleep 1
            printf '%s\n' {second}
            sleep 1
            printf '{{"jsonrpc":"2.0","id":%s,"result":{{"stopReason":"end_turn"}}}}\n' "$request_id" ;;
    esac
done
"#,
            first = shell_words::quote(&update("第一段")),
            second = shell_words::quote(&update("第二段")),
        );
        std::fs::write(workspace.path().join("fake-acp.sh"), script).unwrap();
        request.timeout = Some(Duration::from_secs(5));
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let (_guard, cancel) = tokio::sync::watch::channel(false);
        let answer = DeepSeekHarnessRuntime::with_idle_timeout(Duration::from_millis(1_800))
            .prompt_with_events(request, Some(events), cancel)
            .await
            .unwrap();
        assert_eq!(answer, "第一段第二段");
        let mut completion = None;
        while let Ok(event) = received.try_recv() {
            if let AgentRuntimeEvent::Completed {
                stop_reason,
                complete,
            } = event
            {
                completion = Some((stop_reason, complete));
            }
        }
        assert_eq!(completion, Some(("end_turn".to_string(), true)));
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
                credential_value: None,
                timeout: None,
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
                credential_value: None,
                timeout: None,
            })
            .await
            .expect("real ACP response");

        assert!(answer.contains("连接成功"));
    }
}
