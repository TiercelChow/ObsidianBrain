use super::*;

#[test]
fn test_question_and_each_evidence_survive_large_optional_context() {
    let documents = (0..12)
        .map(|index| ConfigDocument {
            id: format!("config-{index}"),
            knowledge_base_id: None,
            scope: "global".into(),
            name: format!("config-{index}"),
            content_md: "配置内容".repeat(5_000),
            revision: 1,
            updated_at: String::new(),
        })
        .collect::<Vec<_>>();
    let skills = (0..8)
        .map(|index| WikiSkill {
            id: format!("skill-{index}"),
            slug: format!("skill-{index}"),
            name: format!("技能{index}"),
            description: String::new(),
            source_type: "custom".into(),
            status: "ready".into(),
            permissions: vec![],
            requirements: vec![],
            revision: 1,
            instructions: "自定义分析指令".repeat(5_000),
            enabled: true,
            usage_scope: "both".into(),
            updated_at: String::new(),
        })
        .collect::<Vec<_>>();
    let history = (0..8)
        .map(|index| KnowledgeMessage {
            id: format!("message-{index}"),
            role: "user".into(),
            content: "旧问题".repeat(2_000),
            run_id: None,
            evidence: vec![],
            created_at: String::new(),
        })
        .collect::<Vec<_>>();
    let evidence = (0..8)
        .map(|index| KnowledgeEntryDetail {
            entry: KnowledgeEntrySummary {
                id: format!("entry-{index}"),
                knowledge_base_id: "base".into(),
                entry_type: "concept".into(),
                slug: format!("topic-{index}"),
                title: format!("主题{index}"),
                summary: String::new(),
                status: "draft".into(),
                confidence: None,
                source_path: Some(format!("chapter-{index}.md")),
                updated_at: String::new(),
            },
            content_md: format!("独立证据{index} {}", "原始事实".repeat(4_000)),
            aliases: vec![],
            edit_policy: "agent_managed".into(),
            revision: 1,
            citations: vec![],
            claims: vec![],
            relations: vec![],
            versions: vec![],
        })
        .collect::<Vec<_>>();
    let question = format!("当前关键问题：{}问题末尾", "需回答".repeat(500));
    let prompt = build_knowledge_prompt(
        "上下文预算",
        &question,
        &history,
        &documents,
        &skills,
        &evidence,
    );
    assert!(prompt.contains(&question));
    assert!(prompt.chars().count() <= MAX_PROMPT_CHARS);
    for index in 0..8 {
        assert!(prompt.contains(&format!("[S{}] 主题{index}", index + 1)));
        assert!(
            prompt.contains(&format!("独立证据{index}")),
            "lost evidence {index}"
        );
    }
    assert!(prompt.contains("[内容已截断]"));
    assert!(prompt.ends_with("</evidence>\n\n"));
}

#[test]
fn test_compile_preserves_full_batch_with_detailed_skills_and_schema() {
    let skills = vec![WikiSkill {
        id: "skill-book-ingest".into(),
        slug: "book-ingest".into(),
        name: "语义编译".into(),
        description: String::new(),
        source_type: "builtin".into(),
        status: "ready".into(),
        permissions: vec![],
        requirements: vec![],
        revision: 3,
        instructions: include_str!("../../../skills/book-ingest/SKILL.md").into(),
        enabled: true,
        usage_scope: "ingest".into(),
        updated_at: String::new(),
    }];
    let spans = (0..3)
        .map(|index| SourceSpanSnapshot {
            id: format!("span-{index}"),
            source_document_id: format!("doc-{index}"),
            source_version_id: "v1".into(),
            source_path: format!("chapter-{index}.md"),
            heading: Some("长文档".into()),
            line_start: Some(1),
            line_end: Some(500),
            content: format!("{}尾部证据{index}", "资料".repeat(3_000)),
        })
        .collect::<Vec<_>>();
    let prompt = build_semantic_compile_prompt(SemanticCompilePromptInput {
        book_name: "编译预算",
        spans: &spans,
        existing: &[],
        current_candidates: &[],
        documents: &[],
        skills: &skills,
        batch_index: 1,
        batch_count: 1,
    });
    for span in spans {
        assert!(prompt.contains(&span.content));
    }
    assert!(prompt.contains(semantic_output::OUTPUT_SCHEMA));
    assert!(prompt.contains(skills[0].instructions.trim()));
    assert!(prompt.chars().count() <= MAX_PROMPT_CHARS);
}

#[test]
fn test_repair_includes_bounded_failure_data_and_concrete_error() {
    let error = BrainError::KnowledgeValidation("entries/0/confidence 超出范围".into());
    let bad_answer = format!("{{\"confidence\":20}}{}", "异常输出".repeat(5_000));
    let prompt = build_semantic_repair_prompt("原始证据", &error, Some(&bad_answer));
    assert!(prompt.contains("entries/0/confidence"));
    assert!(prompt.contains("confidence\\\":20"));
    assert!(prompt.contains("\"excerpt_truncated\":true"));
    assert!(prompt.chars().count() < 8_000);
}

#[test]
fn test_presentation_prompt_keeps_report_evidence_and_strict_contract() {
    let task = KnowledgeTask {
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
    let report = "## 结论\n即时反馈能降低不确定性。[S1]";

    let prompt = build_presentation_prompt(&task, report, &evidence, &skill);

    assert!(prompt.contains(PRESENTATION_INSTRUCTIONS));
    assert!(prompt.contains(skill.instructions.trim()));
    assert!(prompt.contains(report));
    assert!(prompt.contains("\"citation\":\"S1\""));
    assert!(prompt.contains("只输出符合演示策划合同的单个 JSON 对象"));
    assert!(!prompt.contains("调用工具生成 PPTX"));
}

#[test]
fn test_presentation_repair_is_single_object_and_bounded() {
    let error = BrainError::KnowledgeValidation("slides/2/metric 缺失".into());
    let answer = format!("{{\"schema_version\":\"1.0\"}}{}", "多余输出".repeat(5_000));
    let prompt = build_presentation_repair_prompt("原始合同", &error, &answer);

    assert!(prompt.contains("slides/2/metric"));
    assert!(prompt.contains("唯一一次修复机会"));
    assert!(prompt.contains("不输出补丁、第二个对象、围栏或解释"));
    assert!(prompt.contains("\"excerpt_truncated\":true"));
    assert!(prompt.chars().count() < 10_000);
}

#[test]
fn test_presentation_failure_summary_preserves_completed_research_report() {
    let report = "## 研究结论\n完整报告正文。[S1]";
    let error = BrainError::KnowledgeValidation("演示规格未通过校验".into());

    let summary = build_presentation_failure_summary(report, &error);

    assert!(summary.starts_with("> [!warning] PPTX 生成失败"));
    assert!(summary.contains("演示规格未通过校验"));
    assert!(summary.ends_with(report));
}
