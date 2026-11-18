use super::*;

#[test]
fn test_retryable_harness_failure_only_allows_clear_transient_conditions() {
    for detail in [
        "ACP 调用失败: HTTP 503 Service Unavailable",
        "ACP 调用失败: HTTP/1.1 502 Bad Gateway",
        "ACP 调用失败: status=500",
        "ACP 调用失败: status code 429",
        "ACP 调用失败: 429 rate limit",
        "ACP 调用失败: connection reset by peer",
        "DeepSeek Harness 返回了空回答 (stop_reason=end_turn)",
        "(startup_timeout) DeepSeek Harness 连接阶段超时",
    ] {
        assert!(
            is_retryable_harness_failure(&BrainError::LlmApiError {
                provider: "deepseek_harness".into(),
                detail: detail.into(),
            }),
            "{detail}"
        );
    }
    for detail in [
        "HTTP 401 invalid_api_key",
        "HTTP 403 forbidden",
        "HTTP 401 service unavailable",
        "HTTP 400 invalid_request",
        "HTTP 429 insufficient_quota",
        "DeepSeek Harness 达到输出 token 上限 (stop_reason=max_tokens)",
        "DeepSeek Harness 在 600 秒内没有完成回答",
        "ACP 调用失败: 未知错误",
        "模型输出需要 5000 tokens，但供应商未提供更多空间",
        "日志提到 503 个引用，但请求因格式错误终止",
        "请求预算为 4290 tokens，未返回完整内容",
    ] {
        assert!(
            !is_retryable_harness_failure(&BrainError::LlmApiError {
                provider: "deepseek_harness".into(),
                detail: detail.into(),
            }),
            "{detail}"
        );
    }
    assert!(!is_retryable_harness_failure(
        &BrainError::KnowledgeValidation("JSON 合同不合法".into())
    ));
}

#[test]
fn test_old_run_cleanup_does_not_remove_new_attempt_cancellation() {
    let (sender, receiver) = tokio::sync::watch::channel(false);
    let mut map = HashMap::new();
    map.insert(
        "task".into(),
        ActiveRunCancellation {
            run_id: "new-run".into(),
            attempt: Some(2),
            sender,
        },
    );
    remove_run_cancellation(&mut map, "task", "old-run");
    assert!(map.contains_key("task"));
    let (sender, _) = tokio::sync::watch::channel(false);
    assert!(!register_run_cancellation(
        &mut map,
        "task",
        ActiveRunCancellation {
            run_id: "late-old-run".into(),
            attempt: Some(1),
            sender
        }
    ));
    assert_eq!(map["task"].run_id, "new-run");
    assert!(!*receiver.borrow());
    let (sender, _) = tokio::sync::watch::channel(false);
    assert!(register_run_cancellation(
        &mut map,
        "task",
        ActiveRunCancellation {
            run_id: "newest-run".into(),
            attempt: Some(3),
            sender
        }
    ));
    assert!(*receiver.borrow());
    remove_run_cancellation(&mut map, "task", "new-run");
    assert!(map.contains_key("task"));
    remove_run_cancellation(&mut map, "task", "newest-run");
    assert!(map.is_empty());
}

#[test]
fn test_answer_reference_validation_rejects_unseen_source_numbers_and_ids() {
    let input = serde_json::json!({ "evidence_entry_ids": ["entry-a"] });
    let ledger = vec![AgentEvidenceRef {
        kind: "entry".to_string(),
        object_id: "entry-a".to_string(),
        version_id: "1".to_string(),
        snapshot: serde_json::json!({}),
    }];
    assert!(
        validate_agent_answer_references("结论。[S1] entry_id: entry-a", &input, &ledger).is_ok()
    );
    assert!(validate_agent_answer_references("结论。[S2]", &input, &ledger).is_err());
    assert!(validate_agent_answer_references("entry_id: entry-b", &input, &ledger).is_err());
    assert!(validate_agent_answer_references("span_id: span-b", &input, &ledger).is_err());

    let allocated = vec![AgentEvidenceRef {
        kind: "source_span".into(),
        object_id: "span-b".into(),
        version_id: "version-b".into(),
        snapshot: serde_json::json!({"citation_index": 2}),
    }];
    assert!(validate_agent_answer_references(
        "补证。[S2] span_id: span-b",
        &serde_json::json!({}),
        &allocated
    )
    .is_ok());
    assert!(validate_agent_answer_references("未读。[S3]", &input, &allocated).is_err());
}

#[test]
fn test_presentation_prompt_keeps_report_evidence_and_strict_contract() {
    let mut task = KnowledgeTask {
        id: "task-presentation".into(),
        knowledge_base_id: "base".into(),
        book_name: "设计之书".into(),
        title: "把原则讲成可行动的判断".into(),
        description: "面向产品团队，保留证据边界".into(),
        task_type: "research".into(),
        status: "running".into(),
        result_summary: String::new(),
        deliverable_type: "presentation".into(),
        artifact_state: "pending".into(),
        knowledge_change_state: "none".into(),
        cancel_requested: false,
        external_research_enabled: false,
        external_domains: vec![],
        external_request_limit: 0,
        external_requests_used: 0,
        brief: crate::models::book_wiki::ResearchBrief::default(),
        created_at: String::new(),
        updated_at: String::new(),
    };
    let evidence = vec![KnowledgeEntrySummary {
        id: "entry-1".into(),
        knowledge_base_id: "base".into(),
        entry_type: "concept".into(),
        slug: "feedback".into(),
        title: "即时反馈".into(),
        summary: "反馈应当紧跟用户动作，并说明系统状态。".into(),
        status: "verified".into(),
        confidence: Some(0.9),
        source_path: Some("chapter-3.md".into()),
        updated_at: String::new(),
    }];
    let skill = WikiSkill {
        id: "skill-book-presentation".into(),
        slug: "book-presentation".into(),
        name: "演示策划".into(),
        description: String::new(),
        source_type: "builtin".into(),
        status: "ready".into(),
        permissions: vec![],
        requirements: vec![],
        revision: 3,
        instructions: include_str!("../../../skills/book-presentation/SKILL.md").into(),
        enabled: true,
        usage_scope: "research".into(),
        updated_at: String::new(),
    };
    let report = format!(
        "## 结论\n即时反馈能降低不确定性。[S1]\n{}\n## 最后一章\n尾部关键结论和条件必须进入演示。",
        "完整论证。".repeat(12000)
    );

    let paging = PresentationPagingPlan::new(&task.brief, &[]);
    let originals = BTreeMap::from([(1, "两组实验的平均延迟为 4.0 ms 和 4.1 ms。")]);
    let legacy_prompt =
        build_presentation_prompt(&task, &report, &evidence, &originals, &skill, &paging);
    assert!(!legacy_prompt.contains("<user_confirmed_brief>"));
    task.brief = crate::models::book_wiki::ResearchBrief {
        confirmed: true,
        audience: "specialist".into(),
        purpose: "decision".into(),
        tone: "technical".into(),
        depth: "deep".into(),
        presentation_theme: "midnight".into(),
        presentation_format: "narrative".into(),
        presentation_pagination: Default::default(),
        emphasis: "保留反例".into(),
    };

    let prompt = build_presentation_prompt(&task, &report, &evidence, &originals, &skill, &paging);

    assert!(prompt.contains(PRESENTATION_INSTRUCTIONS));
    assert!(prompt.contains(skill.instructions.trim()));
    assert!(prompt.contains(&report));
    assert!(prompt.contains("尾部关键结论和条件必须进入演示"));
    assert!(prompt.contains("\"citation\":\"S1\""));
    assert!(prompt.contains("\"observed_quantities\":[\"4ms\",\"4.1ms\"]"));
    assert!(prompt.contains("只输出符合演示策划合同的单个 JSON 对象"));
    assert!(prompt.contains("指定主题：midnight"));
    assert!(prompt.contains("保留反例"));
    assert!(prompt.contains("决策路径：先明确选择与判据"));
    assert!(prompt.contains("不要把研究子问题一题一页地搬进演示"));
    assert!(prompt.contains("标题不要照搬研究子问题"));
    assert!(prompt.contains("背景、定义、过程和对照页的 `title` 直接命名具体对象或关系"));
    assert!(!prompt.contains("title` 必须是结论式标题"));
    assert!(prompt.contains("保留术语、公式、变量定义与适用前提"));
    let report_prompt = build_task_prompt(&task.book_name, &task, &[], &[], &[]);
    assert!(report_prompt.contains("决策材料先交代决策与判据"));
    task.brief.purpose = "teach".into();
    let teaching_prompt =
        build_presentation_prompt(&task, &report, &evidence, &originals, &skill, &paging);
    assert!(teaching_prompt.contains("教学路径：先建立必要概念"));
    assert!(!teaching_prompt.contains("决策路径：先明确选择与判据"));
    task.brief.purpose = "reference".into();
    assert!(
        build_presentation_prompt(&task, &report, &evidence, &originals, &skill, &paging)
            .contains("查阅路径：先给可快速定位的结论")
    );
    task.brief.purpose = "understand".into();
    assert!(
        build_task_prompt(&task.book_name, &task, &[], &[], &[]).contains("理解材料先给中心判断")
    );
    assert!(!prompt.contains("调用工具生成 PPTX"));
    let mut spec: PresentationSpec = serde_json::from_value(serde_json::json!({
        "schema_version":"1.0","title":"材料","subtitle":"副标题",
        "audience":"领域专家","core_message":"中心判断","theme":"editorial","slides":[]
    }))
    .unwrap();
    assert!(ensure_task_presentation_theme(&spec, &task).is_err());
    spec.theme = PresentationTheme::Midnight;
    assert!(ensure_task_presentation_theme(&spec, &task).is_ok());

    let question_deck = serde_json::json!({
        "schema_version":"1.0","title":"分层架构材料","subtitle":"机制与条件",
        "audience":"领域专家","core_message":"依赖方向影响变更边界。","theme":"midnight",
        "slides":[
            {"layout":"statement","title":"什么是分层架构？","takeaway":"分层用于限定职责与依赖。","body":["职责划分是起点。"],"citations":["S1"]},
            {"layout":"evidence","title":"为什么需要单向依赖？","takeaway":"反向依赖会扩大变更范围。","body":["依赖方向必须可核对。"],"citations":["S1"]},
            {"layout":"evidence","title":"如何判断边界？","takeaway":"检查变化是否跨越抽象。","body":["以具体变更作为检验。"],"citations":["S1"]},
            {"layout":"summary","title":"把依赖规则带入验收","takeaway":"边界需在实际修改中验证。","body":["保留例外和条件。"],"citations":[]}
        ]
    }).to_string();
    assert!(parse_task_presentation_spec(&question_deck, 1, &task)
        .unwrap_err()
        .to_string()
        .contains("问句标题"));
    task.brief.presentation_format = "qa".into();
    assert!(parse_task_presentation_spec(&question_deck, 1, &task).is_ok());
    task.brief.confirmed = false;
    task.brief.presentation_format = "narrative".into();
    assert!(parse_task_presentation_spec(&question_deck, 1, &task).is_ok());
}

#[test]
fn test_research_preflight_rejects_presentation_theme_for_report() {
    let answer = serde_json::json!({
        "summary":"为读者形成可复核的比较材料",
        "recommended":{"confirmed":true,"audience":"general","purpose":"decision","tone":"analytical","depth":"standard","presentation_theme":"editorial","emphasis":""},
        "focus_decisions":["purpose","presentation_theme"],
        "cautions":[]
    }).to_string();
    assert!(parse_research_preflight(&answer, "report").is_err());
}

#[test]
fn test_research_preflight_presentation_format_requires_presentation_delivery() {
    let answer = serde_json::json!({
        "summary":"制作一份面向讨论的问答式讲解材料",
        "recommended":{"confirmed":false,"audience":"general","purpose":"teach","tone":"narrative","depth":"standard","presentation_theme":"editorial","presentation_format":"qa","emphasis":""},
        "focus_decisions":["presentation_format"],
        "decision_points":[{"field":"presentation_format","question":"逐题讨论还是形成连贯论点？","impact":"决定页面标题与叙事顺序。"}],
        "cautions":[]
    }).to_string();
    let preview = parse_research_preflight(&answer, "presentation").unwrap();
    assert_eq!(preview.recommended.presentation_format, "qa");
    assert!(parse_research_preflight(&answer, "report").is_err());
}

#[test]
fn test_question_slide_title_detection_covers_cjk_and_english_without_prose_false_positives() {
    assert!(is_question_slide_title("分层架构是什么"));
    assert!(is_question_slide_title("Does the evidence support this?"));
    assert!(is_question_slide_title("如何验证适用边界"));
    assert!(!is_question_slide_title("分层架构的依赖边界"));
    assert!(!is_question_slide_title("How teams learn from feedback"));
}

#[test]
fn test_research_preflight_never_marks_model_suggestion_as_user_confirmed() {
    assert!(agent_tools_for_task_type("research_preflight").is_empty());
    let answer = serde_json::json!({
        "summary":"为读者形成可复核的比较材料",
        "recommended":{"confirmed":true,"audience":"specialist","purpose":"decision","tone":"technical","depth":"deep","presentation_theme":"midnight","emphasis":"保留反例"},
        "focus_decisions":["audience","purpose","presentation_theme"],
        "cautions":[]
    }).to_string();
    let parsed = parse_research_preflight(&answer, "presentation").unwrap();
    assert!(!parsed.recommended.confirmed);
    assert_eq!(parsed.recommended.presentation_theme, "midnight");
}

#[test]
fn test_research_preflight_exposes_contextual_decision_questions() {
    let answer = serde_json::json!({
        "summary":"为产品团队准备选型汇报",
        "recommended":{"confirmed":false,"audience":"general","purpose":"decision","tone":"analytical","depth":"standard","presentation_theme":"midnight","emphasis":""},
        "focus_decisions":["audience","purpose"],
        "decision_points":[
            {"field":"audience","question":"汇报对象是技术评审还是业务负责人？","impact":"会改变术语解释和实现细节的比重。"},
            {"field":"purpose","question":"这份材料要帮助选型，还是只说明现状？","impact":"选型会突出判据、方案比较和条件性建议。"}
        ],
        "cautions":[]
    }).to_string();
    let parsed = parse_research_preflight(&answer, "presentation").unwrap();
    assert_eq!(parsed.decision_points.len(), 2);
    assert_eq!(parsed.decision_points[0].field, "audience");
}

#[test]
fn test_research_preflight_rejects_decision_not_in_focus_fields() {
    let answer = serde_json::json!({
        "summary":"制作研究报告",
        "recommended":{"confirmed":false,"audience":"general","purpose":"understand","tone":"analytical","depth":"standard","presentation_theme":"editorial","emphasis":""},
        "focus_decisions":["purpose"],
        "decision_points":[{"field":"tone","question":"使用何种表达？","impact":"改变术语密度。"}],
        "cautions":[]
    }).to_string();
    assert!(parse_research_preflight(&answer, "report").is_err());
}

#[test]
fn test_presentation_repair_is_single_object_and_bounded() {
    let error = BrainError::KnowledgeValidation("slides/2/metric 缺失".into());
    let answer = format!("{{\"schema_version\":\"1.0\"}}{}", "多余输出".repeat(5_000));
    let prompt = build_presentation_repair_prompt("原始合同", &error, &answer);

    assert!(prompt.contains("slides/2/metric"));
    assert!(prompt.contains("按原合同完成本轮修复"));
    assert!(prompt.contains("不输出补丁、第二个对象、围栏或解释"));
    assert!(prompt.contains("\"excerpt_truncated\":true"));
    assert!(prompt.chars().count() < 10_000);
}

#[test]
fn test_research_and_presentation_honor_per_phase_output_allocation() {
    let input = serde_json::json!({"request_max_output_tokens":16384});
    assert_eq!(
        runtime_max_output_tokens_for_invocation("research_preflight", &input),
        Some(16384)
    );
    assert_eq!(
        runtime_max_output_tokens_for_invocation("knowledge_task_research", &input),
        Some(16384)
    );
    assert_eq!(
        runtime_max_output_tokens_for_invocation("knowledge_task_presentation_plan", &input),
        Some(16384)
    );
}

#[test]
fn test_research_resume_only_reuses_capacity_from_the_same_model_route() {
    let profile = RuntimeProfile {
        id: "runtime".into(),
        name: "runtime".into(),
        runtime: "deepseek_harness".into(),
        executable: "deepseek-harness".into(),
        model: "same-model-name".into(),
        provider_id: Some("provider-b".into()),
        provider_config: None,
        enabled: true,
        revision: 1,
        updated_at: String::new(),
    };
    assert!(run_uses_active_model_route(
        &serde_json::json!({"model":"same-model-name","provider_id":"provider-b"}),
        &profile
    ));
    assert!(!run_uses_active_model_route(
        &serde_json::json!({"model":"same-model-name","provider_id":"provider-a"}),
        &profile
    ));
    assert!(!run_uses_active_model_route(
        &serde_json::json!({"model":"older-model","provider_id":"provider-b"}),
        &profile
    ));
    assert!(run_uses_active_model_route(
        &serde_json::json!({"model":"same-model-name"}),
        &profile
    ));
}

#[test]
fn test_research_preflight_output_budget_respects_provider_and_context_headroom() {
    assert_eq!(
        preflight_output_budget(8192, None, None, 1000).unwrap(),
        8192
    );
    assert_eq!(
        preflight_output_budget(16384, Some(12000), Some(20000), 2000).unwrap(),
        12000
    );
    assert_eq!(
        preflight_output_budget(16384, None, Some(10000), 2000).unwrap(),
        6976
    );
    assert!(preflight_output_budget(8192, Some(512), None, 1000).is_err());
    assert!(preflight_output_budget(8192, None, Some(2000), 1200).is_err());
}
