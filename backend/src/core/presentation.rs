use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::BrainError;

const MIN_CONTENT_SLIDES: usize = 4;
const MAX_CONTENT_SLIDES: usize = 12;
const MAX_SLIDE_CHARACTERS: usize = 700;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationSpec {
    pub schema_version: String,
    pub title: String,
    pub subtitle: String,
    pub audience: String,
    pub core_message: String,
    #[serde(default)]
    pub theme: PresentationTheme,
    pub slides: Vec<PresentationSlide>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PresentationTheme {
    #[default]
    Editorial,
    Midnight,
    Sage,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PresentationLayout {
    Statement,
    Split,
    Process,
    Metric,
    Chart,
    Relationship,
    Quote,
    Evidence,
    Summary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationSlide {
    pub layout: PresentationLayout,
    #[serde(default)]
    pub eyebrow: String,
    pub title: String,
    pub takeaway: String,
    #[serde(default)]
    pub body: Vec<String>,
    #[serde(default)]
    pub left: Option<PresentationPanel>,
    #[serde(default)]
    pub right: Option<PresentationPanel>,
    #[serde(default)]
    pub steps: Vec<PresentationStep>,
    #[serde(default)]
    pub metric: Option<PresentationMetric>,
    #[serde(default)]
    pub chart: Option<PresentationChart>,
    #[serde(default)]
    pub relationship: Option<PresentationRelationship>,
    #[serde(default)]
    pub quote: String,
    #[serde(default)]
    pub citations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationPanel {
    pub label: String,
    pub title: String,
    #[serde(default)]
    pub points: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationStep {
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationMetric {
    pub value: String,
    pub label: String,
    pub context: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationChart {
    pub unit: String,
    pub categories: Vec<String>,
    pub values: Vec<f64>,
    #[serde(default)]
    pub highlight_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationRelationship {
    pub center: PresentationRelationshipNode,
    pub related: Vec<PresentationRelatedNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationRelationshipNode {
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PresentationRelatedNode {
    pub relation: String,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PresentationPlanValidation {
    pub content_slide_count: usize,
    pub layout_count: usize,
    pub cited_slide_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PresentationValidation {
    pub slide_count: usize,
    pub layout_count: usize,
    pub package_part_count: usize,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PresentationQualityCheck {
    pub code: String,
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PresentationQualityReport {
    pub schema_version: String,
    pub slide_count: usize,
    pub content_slide_count: usize,
    pub theme: String,
    pub layout_count: usize,
    pub layout_distribution: BTreeMap<String, usize>,
    pub cited_slide_count: usize,
    pub citation_coverage_percent: usize,
    pub package_part_count: usize,
    pub editable_text_and_shapes: bool,
    pub checks: Vec<PresentationQualityCheck>,
    pub summary: String,
}

pub fn parse_presentation_spec(
    answer: &str,
    evidence_count: usize,
) -> Result<PresentationSpec, BrainError> {
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Err(validation_error("演示策划返回了空结果"));
    }
    let mut deserializer = serde_json::Deserializer::from_str(trimmed);
    let mut spec = PresentationSpec::deserialize(&mut deserializer)
        .map_err(|error| validation_error(&format!("演示策划 JSON 解析失败: {error}")))?;
    deserializer
        .end()
        .map_err(|error| validation_error(&format!("演示策划包含多余内容: {error}")))?;
    for slide in &mut spec.slides {
        slide.citations = slide
            .citations
            .iter()
            .map(|citation| normalize_citation(citation))
            .collect::<Result<Vec<_>, _>>()?;
    }
    validate_presentation_spec(&spec, evidence_count)?;
    Ok(spec)
}

pub fn validate_presentation_spec(
    spec: &PresentationSpec,
    evidence_count: usize,
) -> Result<PresentationPlanValidation, BrainError> {
    if spec.schema_version != "1.0" {
        return Err(validation_error("schema_version 必须为 1.0"));
    }
    validate_text("title", &spec.title, 80, false)?;
    validate_text("subtitle", &spec.subtitle, 120, false)?;
    validate_text("audience", &spec.audience, 120, false)?;
    validate_text("core_message", &spec.core_message, 180, false)?;
    if !(MIN_CONTENT_SLIDES..=MAX_CONTENT_SLIDES).contains(&spec.slides.len()) {
        return Err(validation_error(&format!(
            "slides 必须包含 {MIN_CONTENT_SLIDES} 到 {MAX_CONTENT_SLIDES} 页内容"
        )));
    }

    let mut layouts = HashSet::new();
    let mut slide_titles = HashSet::new();
    let mut cited_slide_count = 0;
    for (index, slide) in spec.slides.iter().enumerate() {
        let field = format!("slides[{index}]");
        validate_text(&format!("{field}.eyebrow"), &slide.eyebrow, 24, true)?;
        validate_text(&format!("{field}.title"), &slide.title, 60, false)?;
        validate_text(&format!("{field}.takeaway"), &slide.takeaway, 140, false)?;
        let title_identity = heading_identity(&slide.title);
        if !slide_titles.insert(title_identity.clone()) {
            return Err(validation_error(&format!(
                "{field}.title 与其他页面标题重复"
            )));
        }
        if title_identity == heading_identity(&slide.takeaway) {
            return Err(validation_error(&format!(
                "{field}.takeaway 不应重复本页标题"
            )));
        }
        if slide.body.len() > 5 {
            return Err(validation_error(&format!("{field}.body 最多 5 条")));
        }
        for (item_index, item) in slide.body.iter().enumerate() {
            validate_text(&format!("{field}.body[{item_index}]"), item, 160, false)?;
        }
        if slide.citations.len() > 4 {
            return Err(validation_error(&format!("{field}.citations 最多 4 个")));
        }
        for citation in &slide.citations {
            let ordinal = citation
                .strip_prefix('S')
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| validation_error(&format!("{field} 引用 {citation} 格式无效")))?;
            if ordinal == 0 || ordinal > evidence_count {
                return Err(validation_error(&format!(
                    "{field} 引用 {citation} 不在当前证据 S1..S{evidence_count} 中"
                )));
            }
        }
        cited_slide_count += usize::from(!slide.citations.is_empty());
        validate_layout_payload(&field, slide)?;
        if slide_character_count(slide) > MAX_SLIDE_CHARACTERS {
            return Err(validation_error(&format!("{field} 文本过载")));
        }
        layouts.insert(slide.layout);
    }
    if spec.slides.len() >= 5 && layouts.len() < 3 {
        return Err(validation_error(
            "5 页以上的演示文稿至少需要 3 种布局，避免整篇重复",
        ));
    }
    let required_cited_slides = spec.slides.len().saturating_sub(1).max(1);
    if evidence_count > 0 && cited_slide_count < required_cited_slides {
        return Err(validation_error(&format!(
            "除收束页外的内容页都应绑定证据：当前 {cited_slide_count}/{} 页有引用",
            spec.slides.len()
        )));
    }
    Ok(PresentationPlanValidation {
        content_slide_count: spec.slides.len(),
        layout_count: layouts.len(),
        cited_slide_count,
    })
}

fn heading_identity(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(&['。', '.', '？', '?', '！', '!'][..])
        .trim()
        .to_lowercase()
}

fn validate_layout_payload(field: &str, slide: &PresentationSlide) -> Result<(), BrainError> {
    match slide.layout {
        PresentationLayout::Split => {
            validate_panel(
                field,
                "left",
                slide
                    .left
                    .as_ref()
                    .ok_or_else(|| validation_error(&format!("{field}.left 不能为空")))?,
            )?;
            validate_panel(
                field,
                "right",
                slide
                    .right
                    .as_ref()
                    .ok_or_else(|| validation_error(&format!("{field}.right 不能为空")))?,
            )?;
        }
        PresentationLayout::Process => {
            if !(3..=5).contains(&slide.steps.len()) {
                return Err(validation_error(&format!(
                    "{field}.steps 必须包含 3 到 5 步"
                )));
            }
            for (index, step) in slide.steps.iter().enumerate() {
                validate_text(
                    &format!("{field}.steps[{index}].title"),
                    &step.title,
                    32,
                    false,
                )?;
                validate_text(
                    &format!("{field}.steps[{index}].detail"),
                    &step.detail,
                    90,
                    false,
                )?;
            }
        }
        PresentationLayout::Metric => {
            let metric = slide
                .metric
                .as_ref()
                .ok_or_else(|| validation_error(&format!("{field}.metric 不能为空")))?;
            validate_text(&format!("{field}.metric.value"), &metric.value, 24, false)?;
            validate_text(&format!("{field}.metric.label"), &metric.label, 60, false)?;
            validate_text(
                &format!("{field}.metric.context"),
                &metric.context,
                160,
                false,
            )?;
        }
        PresentationLayout::Chart => {
            let chart = slide
                .chart
                .as_ref()
                .ok_or_else(|| validation_error(&format!("{field}.chart 不能为空")))?;
            validate_text(&format!("{field}.chart.unit"), &chart.unit, 16, true)?;
            if !(2..=6).contains(&chart.categories.len()) {
                return Err(validation_error(&format!(
                    "{field}.chart.categories 必须包含 2 到 6 项"
                )));
            }
            if chart.values.len() != chart.categories.len() {
                return Err(validation_error(&format!(
                    "{field}.chart.values 必须与 categories 一一对应"
                )));
            }
            for (index, category) in chart.categories.iter().enumerate() {
                validate_text(
                    &format!("{field}.chart.categories[{index}]"),
                    category,
                    24,
                    false,
                )?;
            }
            for (index, value) in chart.values.iter().enumerate() {
                if !value.is_finite() {
                    return Err(validation_error(&format!(
                        "{field}.chart.values[{index}] 必须是有限数值"
                    )));
                }
                if *value < 0.0 {
                    return Err(validation_error(&format!(
                        "{field}.chart.values[{index}] 不能为负数"
                    )));
                }
            }
            if chart.values.iter().all(|value| *value == 0.0) {
                return Err(validation_error(&format!(
                    "{field}.chart.values 不能全部为零"
                )));
            }
            if chart
                .highlight_index
                .is_some_and(|index| index >= chart.values.len())
            {
                return Err(validation_error(&format!(
                    "{field}.chart.highlight_index 超出数据范围"
                )));
            }
        }
        PresentationLayout::Relationship => {
            let relationship = slide
                .relationship
                .as_ref()
                .ok_or_else(|| validation_error(&format!("{field}.relationship 不能为空")))?;
            validate_text(
                &format!("{field}.relationship.center.title"),
                &relationship.center.title,
                32,
                false,
            )?;
            validate_text(
                &format!("{field}.relationship.center.detail"),
                &relationship.center.detail,
                72,
                false,
            )?;
            if !(2..=4).contains(&relationship.related.len()) {
                return Err(validation_error(&format!(
                    "{field}.relationship.related 必须包含 2 到 4 项"
                )));
            }
            for (index, related) in relationship.related.iter().enumerate() {
                validate_text(
                    &format!("{field}.relationship.related[{index}].relation"),
                    &related.relation,
                    16,
                    false,
                )?;
                validate_text(
                    &format!("{field}.relationship.related[{index}].title"),
                    &related.title,
                    32,
                    false,
                )?;
                validate_text(
                    &format!("{field}.relationship.related[{index}].detail"),
                    &related.detail,
                    72,
                    false,
                )?;
            }
        }
        PresentationLayout::Quote => {
            validate_text(&format!("{field}.quote"), &slide.quote, 240, false)?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_panel(field: &str, side: &str, panel: &PresentationPanel) -> Result<(), BrainError> {
    validate_text(&format!("{field}.{side}.label"), &panel.label, 24, false)?;
    validate_text(&format!("{field}.{side}.title"), &panel.title, 50, false)?;
    if panel.points.is_empty() || panel.points.len() > 4 {
        return Err(validation_error(&format!(
            "{field}.{side}.points 必须包含 1 到 4 条"
        )));
    }
    for (index, point) in panel.points.iter().enumerate() {
        validate_text(
            &format!("{field}.{side}.points[{index}]"),
            point,
            120,
            false,
        )?;
    }
    Ok(())
}

fn validate_text(
    field: &str,
    value: &str,
    maximum: usize,
    allow_empty: bool,
) -> Result<(), BrainError> {
    let length = value.trim().chars().count();
    if !allow_empty && length == 0 {
        return Err(validation_error(&format!("{field} 不能为空")));
    }
    if length > maximum {
        return Err(validation_error(&format!(
            "{field} 过长（{length} 字，上限 {maximum}）"
        )));
    }
    Ok(())
}

fn normalize_citation(value: &str) -> Result<String, BrainError> {
    let normalized = value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_uppercase();
    if normalized
        .strip_prefix('S')
        .is_some_and(|ordinal| !ordinal.is_empty() && ordinal.chars().all(|ch| ch.is_ascii_digit()))
    {
        Ok(normalized)
    } else {
        Err(validation_error(&format!(
            "引用 {value} 必须使用 S<n> 格式"
        )))
    }
}

fn slide_character_count(slide: &PresentationSlide) -> usize {
    let mut values = vec![
        slide.eyebrow.as_str(),
        slide.title.as_str(),
        slide.takeaway.as_str(),
        slide.quote.as_str(),
    ];
    values.extend(slide.body.iter().map(String::as_str));
    for panel in [slide.left.as_ref(), slide.right.as_ref()]
        .into_iter()
        .flatten()
    {
        values.extend([panel.label.as_str(), panel.title.as_str()]);
        values.extend(panel.points.iter().map(String::as_str));
    }
    for step in &slide.steps {
        values.extend([step.title.as_str(), step.detail.as_str()]);
    }
    if let Some(metric) = &slide.metric {
        values.extend([
            metric.value.as_str(),
            metric.label.as_str(),
            metric.context.as_str(),
        ]);
    }
    if let Some(chart) = &slide.chart {
        values.push(chart.unit.as_str());
        values.extend(chart.categories.iter().map(String::as_str));
    }
    if let Some(relationship) = &slide.relationship {
        values.extend([
            relationship.center.title.as_str(),
            relationship.center.detail.as_str(),
        ]);
        for related in &relationship.related {
            values.extend([
                related.relation.as_str(),
                related.title.as_str(),
                related.detail.as_str(),
            ]);
        }
    }
    values.into_iter().map(|value| value.chars().count()).sum()
}

fn validation_error(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(format!("演示文稿校验失败: {message}"))
}

pub fn render_pptx(spec: &PresentationSpec, output: &Path) -> Result<(), BrainError> {
    let plan = validate_presentation_spec(spec, usize::MAX)?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staging = output.with_extension("pptx.part");
    let file = File::create(&staging)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let slide_count = spec.slides.len() + 1;

    for (name, content) in [
        (
            "[Content_Types].xml".to_string(),
            content_types(slide_count),
        ),
        ("_rels/.rels".to_string(), ROOT_RELS.to_string()),
        ("docProps/app.xml".to_string(), app_properties(slide_count)),
        (
            "docProps/core.xml".to_string(),
            core_properties(&spec.title),
        ),
        (
            "ppt/presentation.xml".to_string(),
            presentation_xml(slide_count),
        ),
        (
            "ppt/_rels/presentation.xml.rels".to_string(),
            presentation_relationships(slide_count),
        ),
        ("ppt/theme/theme1.xml".to_string(), THEME_XML.to_string()),
        (
            "ppt/slideMasters/slideMaster1.xml".to_string(),
            MASTER_XML.to_string(),
        ),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels".to_string(),
            MASTER_RELS.to_string(),
        ),
        (
            "ppt/slideLayouts/slideLayout1.xml".to_string(),
            LAYOUT_XML.to_string(),
        ),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_string(),
            LAYOUT_RELS.to_string(),
        ),
    ] {
        write_part(&mut archive, options, &name, &content)?;
    }

    let palette = Palette::for_theme(spec.theme);
    let mut slides = vec![cover_slide_xml(spec, &palette, slide_count)];
    slides.extend(
        spec.slides
            .iter()
            .enumerate()
            .map(|(index, slide)| content_slide_xml(slide, &palette, index + 2, slide_count)),
    );
    for (offset, xml) in slides.into_iter().enumerate() {
        let number = offset + 1;
        write_part(
            &mut archive,
            options,
            &format!("ppt/slides/slide{number}.xml"),
            &xml,
        )?;
        write_part(
            &mut archive,
            options,
            &format!("ppt/slides/_rels/slide{number}.xml.rels"),
            SLIDE_RELS,
        )?;
    }
    archive
        .finish()
        .map_err(|error| BrainError::Internal(format!("PPTX 归档完成失败: {error}")))?;
    std::fs::rename(staging, output)?;
    tracing::debug!(slides = slide_count, layouts = plan.layout_count, theme = ?spec.theme, "已渲染结构化演示文稿");
    Ok(())
}

pub fn validate_pptx(path: &Path) -> Result<PresentationValidation, BrainError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| BrainError::Internal(format!("PPTX 文件结构无效: {error}")))?;
    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "ppt/presentation.xml",
        "ppt/theme/theme1.xml",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/slides/slide1.xml",
    ] {
        archive
            .by_name(required)
            .map_err(|_| BrainError::Internal(format!("PPTX 缺少必要部件: {required}")))?;
    }
    let mut slide_count = 0;
    let mut layouts = HashSet::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| BrainError::Internal(format!("PPTX 部件读取失败: {error}")))?;
        let name = file.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_count += 1;
            let mut xml = String::new();
            file.read_to_string(&mut xml)?;
            for layout in [
                "cover",
                "statement",
                "split",
                "process",
                "metric",
                "chart",
                "relationship",
                "quote",
                "evidence",
                "summary",
            ] {
                if xml.contains(&format!("ppt-layout:{layout}")) {
                    layouts.insert(layout);
                }
            }
        }
    }
    if slide_count < MIN_CONTENT_SLIDES + 1 {
        return Err(BrainError::Internal(format!(
            "PPTX 可显示页面不足：{slide_count} 页"
        )));
    }
    let package_part_count = archive.len();
    Ok(PresentationValidation {
        slide_count,
        layout_count: layouts.len(),
        package_part_count,
        message: format!(
            "PPTX 文件结构完整，共 {slide_count} 页，使用 {} 种构图，文本与信息图可编辑",
            layouts.len()
        ),
    })
}

pub fn build_presentation_quality_report(
    spec: &PresentationSpec,
    plan: &PresentationPlanValidation,
    package: &PresentationValidation,
) -> PresentationQualityReport {
    let mut layout_distribution = BTreeMap::new();
    for slide in &spec.slides {
        *layout_distribution
            .entry(layout_name(slide.layout).to_string())
            .or_insert(0) += 1;
    }
    let citation_coverage_percent = (plan.cited_slide_count * 100)
        .checked_div(plan.content_slide_count)
        .unwrap_or(0);
    let summary = format!(
        "结构校验通过 · {} 页 · {} 种构图 · 引用覆盖 {citation_coverage_percent}%",
        package.slide_count, plan.layout_count
    );
    PresentationQualityReport {
        schema_version: "1.0".to_string(),
        slide_count: package.slide_count,
        content_slide_count: plan.content_slide_count,
        theme: theme_name(spec.theme).to_string(),
        layout_count: plan.layout_count,
        layout_distribution,
        cited_slide_count: plan.cited_slide_count,
        citation_coverage_percent,
        package_part_count: package.package_part_count,
        editable_text_and_shapes: true,
        checks: vec![
            PresentationQualityCheck {
                code: "spec_contract".to_string(),
                label: "演示规格".to_string(),
                passed: true,
                detail: format!("{} 页内容通过字段与密度校验", plan.content_slide_count),
            },
            PresentationQualityCheck {
                code: "citation_coverage".to_string(),
                label: "证据覆盖".to_string(),
                passed: true,
                detail: format!(
                    "{} / {} 页绑定数据库证据",
                    plan.cited_slide_count, plan.content_slide_count
                ),
            },
            PresentationQualityCheck {
                code: "layout_variety".to_string(),
                label: "构图多样性".to_string(),
                passed: true,
                detail: format!("使用 {} 种内容构图", plan.layout_count),
            },
            PresentationQualityCheck {
                code: "pptx_package".to_string(),
                label: "PPTX 包结构".to_string(),
                passed: true,
                detail: format!("已读取并检查 {} 个 OOXML 部件", package.package_part_count),
            },
            PresentationQualityCheck {
                code: "editable_objects".to_string(),
                label: "可编辑对象".to_string(),
                passed: true,
                detail: "正文、数据图和关系图均由文本框与基础形状组成".to_string(),
            },
        ],
        summary,
    }
}

#[derive(Clone, Copy)]
struct Palette {
    background: &'static str,
    text: &'static str,
    muted: &'static str,
    accent: &'static str,
    accent_two: &'static str,
    accent_soft: &'static str,
    inverse: &'static str,
}

impl Palette {
    fn for_theme(theme: PresentationTheme) -> Self {
        match theme {
            PresentationTheme::Editorial => Self {
                background: "F4F0E8",
                text: "1D1D1F",
                muted: "68645F",
                accent: "5A4AE3",
                accent_two: "D85C41",
                accent_soft: "E5E0FB",
                inverse: "FFFFFF",
            },
            PresentationTheme::Midnight => Self {
                background: "101522",
                text: "F5F7FB",
                muted: "A8B0C2",
                accent: "72D5F2",
                accent_two: "B18CFF",
                accent_soft: "25334B",
                inverse: "101522",
            },
            PresentationTheme::Sage => Self {
                background: "F1F1E7",
                text: "17251D",
                muted: "5F6A62",
                accent: "2C6B4F",
                accent_two: "C9822D",
                accent_soft: "DDE9DF",
                inverse: "FFFFFF",
            },
        }
    }
}

fn cover_slide_xml(spec: &PresentationSpec, palette: &Palette, total: usize) -> String {
    let shapes = [
        shape(
            2,
            "ppt-layout:cover",
            "rect",
            8_950_000,
            0,
            3_242_000,
            6_858_000,
            palette.accent,
            palette.accent,
        ),
        text_box(
            3,
            "品牌",
            720_000,
            600_000,
            6_900_000,
            360_000,
            "OBSIDIANBRAIN · BOOK WIKI",
            1_300,
            palette.accent,
            true,
            "l",
        ),
        text_box(
            4,
            "封面标题",
            720_000,
            1_420_000,
            7_500_000,
            1_650_000,
            &spec.title,
            4_200,
            palette.text,
            true,
            "l",
        ),
        text_box(
            5,
            "核心信息",
            760_000,
            3_330_000,
            6_900_000,
            1_050_000,
            &spec.core_message,
            2_150,
            palette.muted,
            false,
            "l",
        ),
        text_box(
            6,
            "副标题",
            760_000,
            5_530_000,
            7_200_000,
            360_000,
            &spec.subtitle,
            1_250,
            palette.muted,
            false,
            "l",
        ),
        text_box(
            7,
            "受众",
            9_360_000,
            4_850_000,
            2_200_000,
            1_050_000,
            &format!("为谁而讲\n{}", spec.audience),
            1_300,
            palette.inverse,
            true,
            "l",
        ),
    ]
    .join("");
    slide_document(palette.background, &shapes, 1, total)
}

fn content_slide_xml(
    slide: &PresentationSlide,
    palette: &Palette,
    index: usize,
    total: usize,
) -> String {
    let mut shapes = String::new();
    shapes.push_str(&text_box(
        2,
        &format!("ppt-layout:{}", layout_name(slide.layout)),
        720_000,
        330_000,
        3_500_000,
        300_000,
        if slide.eyebrow.trim().is_empty() {
            "BOOK WIKI"
        } else {
            &slide.eyebrow
        },
        1_050,
        palette.accent,
        true,
        "l",
    ));
    shapes.push_str(&text_box(
        3,
        "页面标题",
        720_000,
        700_000,
        10_700_000,
        800_000,
        &slide.title,
        3_200,
        palette.text,
        true,
        "l",
    ));
    match slide.layout {
        PresentationLayout::Statement => render_statement(&mut shapes, slide, palette),
        PresentationLayout::Split => render_split(&mut shapes, slide, palette),
        PresentationLayout::Process => render_process(&mut shapes, slide, palette),
        PresentationLayout::Metric => render_metric(&mut shapes, slide, palette),
        PresentationLayout::Chart => render_chart(&mut shapes, slide, palette),
        PresentationLayout::Relationship => render_relationship(&mut shapes, slide, palette),
        PresentationLayout::Quote => render_quote(&mut shapes, slide, palette),
        PresentationLayout::Evidence => render_evidence(&mut shapes, slide, palette),
        PresentationLayout::Summary => render_summary(&mut shapes, slide, palette),
    }
    shapes.push_str(&citation_footer(slide, palette));
    slide_document(palette.background, &shapes, index, total)
}

fn render_statement(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    out.push_str(&shape(
        4,
        "强调线",
        "rect",
        760_000,
        1_720_000,
        90_000,
        2_250_000,
        p.accent_two,
        p.accent_two,
    ));
    out.push_str(&text_box(
        5,
        "核心观点",
        1_120_000,
        1_650_000,
        9_850_000,
        1_550_000,
        &slide.takeaway,
        2_800,
        p.text,
        true,
        "l",
    ));
    out.push_str(&bullet_box(
        6,
        "补充要点",
        1_140_000,
        3_420_000,
        9_650_000,
        1_700_000,
        &slide.body,
        1_700,
        p.muted,
    ));
}

fn render_split(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    out.push_str(&text_box(
        4,
        "页面结论",
        760_000,
        1_500_000,
        10_500_000,
        530_000,
        &slide.takeaway,
        1_650,
        p.muted,
        false,
        "l",
    ));
    out.push_str(&shape(
        5,
        "分隔线",
        "rect",
        6_050_000,
        2_230_000,
        18_000,
        2_900_000,
        p.accent_soft,
        p.accent_soft,
    ));
    for (offset, panel) in [
        (760_000, slide.left.as_ref()),
        (6_410_000, slide.right.as_ref()),
    ] {
        if let Some(panel) = panel {
            let id = if offset < 1_000_000 { 10 } else { 20 };
            let accent = if id == 10 { p.accent } else { p.accent_two };
            out.push_str(&text_box(
                id,
                "分栏标签",
                offset,
                2_200_000,
                4_900_000,
                300_000,
                &panel.label,
                1_050,
                accent,
                true,
                "l",
            ));
            out.push_str(&text_box(
                id + 1,
                "分栏标题",
                offset,
                2_560_000,
                4_900_000,
                650_000,
                &panel.title,
                2_050,
                p.text,
                true,
                "l",
            ));
            out.push_str(&bullet_box(
                id + 2,
                "分栏要点",
                offset,
                3_350_000,
                4_850_000,
                1_850_000,
                &panel.points,
                1_500,
                p.muted,
            ));
        }
    }
}

fn render_process(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    out.push_str(&text_box(
        4,
        "页面结论",
        760_000,
        1_500_000,
        10_600_000,
        520_000,
        &slide.takeaway,
        1_650,
        p.muted,
        false,
        "l",
    ));
    let width = 10_400_000 / slide.steps.len().max(1);
    out.push_str(&shape(
        5,
        "流程连线",
        "rect",
        1_080_000,
        2_710_000,
        9_300_000,
        22_000,
        p.accent_soft,
        p.accent_soft,
    ));
    for (ordinal, step) in slide.steps.iter().enumerate() {
        let x = 760_000 + ordinal * width;
        let id = 10 + ordinal * 3;
        out.push_str(&shape(
            id,
            "步骤编号",
            "ellipse",
            x + 270_000,
            2_370_000,
            680_000,
            680_000,
            p.accent,
            p.accent,
        ));
        out.push_str(&text_box(
            id + 1,
            "步骤数字",
            x + 270_000,
            2_370_000,
            680_000,
            680_000,
            &(ordinal + 1).to_string(),
            1_700,
            p.inverse,
            true,
            "ctr",
        ));
        out.push_str(&text_box(
            id + 2,
            "步骤内容",
            x,
            3_300_000,
            width.saturating_sub(160_000),
            1_550_000,
            &format!("{}\n{}", step.title, step.detail),
            1_350,
            p.text,
            true,
            "l",
        ));
    }
}

fn render_metric(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    let Some(metric) = &slide.metric else { return };
    out.push_str(&text_box(
        4,
        "数字",
        760_000,
        1_700_000,
        4_150_000,
        1_500_000,
        &metric.value,
        5_400,
        p.accent,
        true,
        "l",
    ));
    out.push_str(&text_box(
        5,
        "数字标签",
        800_000,
        3_130_000,
        3_950_000,
        520_000,
        &metric.label,
        1_750,
        p.text,
        true,
        "l",
    ));
    out.push_str(&text_box(
        6,
        "数字语境",
        800_000,
        3_780_000,
        3_950_000,
        1_050_000,
        &metric.context,
        1_350,
        p.muted,
        false,
        "l",
    ));
    out.push_str(&shape(
        7,
        "数字分隔",
        "rect",
        5_180_000,
        1_850_000,
        18_000,
        3_000_000,
        p.accent_soft,
        p.accent_soft,
    ));
    out.push_str(&text_box(
        8,
        "数字结论",
        5_650_000,
        1_750_000,
        5_650_000,
        1_050_000,
        &slide.takeaway,
        2_200,
        p.text,
        true,
        "l",
    ));
    out.push_str(&bullet_box(
        9,
        "数字要点",
        5_680_000,
        3_050_000,
        5_400_000,
        1_850_000,
        &slide.body,
        1_450,
        p.muted,
    ));
}

fn render_chart(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    let Some(chart) = &slide.chart else { return };
    out.push_str(&text_box(
        4,
        "图表结论",
        760_000,
        1_480_000,
        10_500_000,
        500_000,
        &slide.takeaway,
        1_600,
        p.muted,
        false,
        "l",
    ));
    let maximum = chart.values.iter().copied().fold(0.0_f64, f64::max);
    let row_height = 3_200_000 / chart.values.len().max(1);
    for (index, (category, value)) in chart.categories.iter().zip(chart.values.iter()).enumerate() {
        let y = 2_100_000 + index * row_height;
        let bar_width = ((value / maximum) * 6_250_000.0).round() as usize;
        let color = if chart.highlight_index == Some(index) {
            p.accent_two
        } else {
            p.accent
        };
        let id = 10 + index * 5;
        out.push_str(&text_box(
            id,
            "图表类别",
            780_000,
            y,
            1_850_000,
            row_height.saturating_sub(90_000),
            category,
            1_250,
            p.text,
            true,
            "l",
        ));
        out.push_str(&shape(
            id + 1,
            "图表刻度轨",
            "roundRect",
            2_750_000,
            y + 70_000,
            6_250_000,
            row_height.saturating_sub(230_000),
            p.accent_soft,
            p.accent_soft,
        ));
        out.push_str(&shape(
            id + 2,
            "图表数据条",
            "roundRect",
            2_750_000,
            y + 70_000,
            bar_width.max(24_000),
            row_height.saturating_sub(230_000),
            color,
            color,
        ));
        out.push_str(&text_box(
            id + 3,
            "图表数值",
            9_250_000,
            y,
            1_650_000,
            row_height.saturating_sub(90_000),
            &format!("{}{}", format_chart_value(*value), chart.unit),
            1_350,
            color,
            true,
            "r",
        ));
    }
}

fn render_relationship(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    let Some(relationship) = &slide.relationship else {
        return;
    };
    out.push_str(&text_box(
        4,
        "关系结论",
        760_000,
        1_480_000,
        10_500_000,
        500_000,
        &slide.takeaway,
        1_600,
        p.muted,
        false,
        "l",
    ));
    let row_height = 3_250_000 / relationship.related.len().max(1);
    for (index, related) in relationship.related.iter().enumerate() {
        let y = 2_090_000 + index * row_height;
        let id = 20 + index * 5;
        out.push_str(&shape(
            id,
            "关系连接线",
            "rect",
            3_830_000,
            y + row_height / 2,
            1_720_000,
            18_000,
            p.accent_soft,
            p.accent_soft,
        ));
        out.push_str(&text_box(
            id + 1,
            "关系标签",
            4_020_000,
            y + row_height / 2 - 310_000,
            1_350_000,
            260_000,
            &related.relation,
            900,
            p.accent_two,
            true,
            "ctr",
        ));
        out.push_str(&shape(
            id + 2,
            "相关概念",
            "roundRect",
            5_520_000,
            y,
            5_500_000,
            row_height.saturating_sub(150_000),
            p.accent_soft,
            p.accent_soft,
        ));
        out.push_str(&text_box(
            id + 3,
            "相关概念文字",
            5_850_000,
            y + 80_000,
            4_900_000,
            row_height.saturating_sub(300_000),
            &format!("{}\n{}", related.title, related.detail),
            1_250,
            p.text,
            true,
            "l",
        ));
    }
    out.push_str(&shape(
        10,
        "中心概念",
        "roundRect",
        760_000,
        2_550_000,
        3_080_000,
        1_650_000,
        p.accent,
        p.accent,
    ));
    out.push_str(&text_box(
        11,
        "中心概念文字",
        1_060_000,
        2_760_000,
        2_480_000,
        1_180_000,
        &format!(
            "{}\n{}",
            relationship.center.title, relationship.center.detail
        ),
        1_450,
        p.inverse,
        true,
        "l",
    ));
}

fn format_chart_value(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn render_quote(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    out.push_str(&text_box(
        4,
        "引号",
        760_000,
        1_500_000,
        1_000_000,
        1_000_000,
        "“",
        6_800,
        p.accent_two,
        true,
        "l",
    ));
    out.push_str(&text_box(
        5,
        "引语",
        1_650_000,
        1_720_000,
        8_900_000,
        2_200_000,
        &slide.quote,
        2_650,
        p.text,
        true,
        "l",
    ));
    out.push_str(&text_box(
        6,
        "引语解读",
        1_700_000,
        4_150_000,
        8_700_000,
        900_000,
        &slide.takeaway,
        1_500,
        p.muted,
        false,
        "l",
    ));
}

fn render_evidence(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    out.push_str(&text_box(
        4,
        "证据结论",
        760_000,
        1_520_000,
        7_500_000,
        1_000_000,
        &slide.takeaway,
        2_250,
        p.text,
        true,
        "l",
    ));
    out.push_str(&bullet_box(
        5,
        "证据要点",
        790_000,
        2_820_000,
        7_350_000,
        2_150_000,
        &slide.body,
        1_550,
        p.muted,
    ));
    out.push_str(&shape(
        6,
        "证据轨",
        "roundRect",
        8_720_000,
        1_620_000,
        2_550_000,
        3_520_000,
        p.accent_soft,
        p.accent_soft,
    ));
    out.push_str(&text_box(
        7,
        "证据轨标题",
        9_060_000,
        1_940_000,
        1_850_000,
        420_000,
        "EVIDENCE",
        1_050,
        p.accent,
        true,
        "l",
    ));
    out.push_str(&text_box(
        8,
        "证据编号",
        9_060_000,
        2_650_000,
        1_850_000,
        1_250_000,
        &slide.citations.join("\n"),
        2_000,
        p.text,
        true,
        "l",
    ));
}

fn render_summary(out: &mut String, slide: &PresentationSlide, p: &Palette) {
    out.push_str(&text_box(
        4,
        "收束结论",
        760_000,
        1_520_000,
        10_300_000,
        930_000,
        &slide.takeaway,
        2_350,
        p.text,
        true,
        "l",
    ));
    for (index, item) in slide.body.iter().take(4).enumerate() {
        let y = 2_760_000 + index * 650_000;
        out.push_str(&shape(
            10 + index * 3,
            "收束编号",
            "ellipse",
            810_000,
            y,
            510_000,
            510_000,
            p.accent,
            p.accent,
        ));
        out.push_str(&text_box(
            11 + index * 3,
            "收束编号文本",
            810_000,
            y,
            510_000,
            510_000,
            &format!("{:02}", index + 1),
            1_300,
            p.inverse,
            true,
            "ctr",
        ));
        out.push_str(&text_box(
            12 + index * 3,
            "收束要点",
            1_560_000,
            y - 20_000,
            9_200_000,
            560_000,
            item,
            1_600,
            p.muted,
            false,
            "l",
        ));
    }
}

fn citation_footer(slide: &PresentationSlide, p: &Palette) -> String {
    let value = if slide.citations.is_empty() {
        "结论页 · 完整依据见研究报告".to_string()
    } else {
        format!("依据  {}", slide.citations.join("  ·  "))
    };
    text_box(
        90,
        "引用页脚",
        760_000,
        6_150_000,
        9_700_000,
        260_000,
        &value,
        850,
        p.muted,
        false,
        "l",
    )
}

fn layout_name(layout: PresentationLayout) -> &'static str {
    match layout {
        PresentationLayout::Statement => "statement",
        PresentationLayout::Split => "split",
        PresentationLayout::Process => "process",
        PresentationLayout::Metric => "metric",
        PresentationLayout::Chart => "chart",
        PresentationLayout::Relationship => "relationship",
        PresentationLayout::Quote => "quote",
        PresentationLayout::Evidence => "evidence",
        PresentationLayout::Summary => "summary",
    }
}

fn theme_name(theme: PresentationTheme) -> &'static str {
    match theme {
        PresentationTheme::Editorial => "editorial",
        PresentationTheme::Midnight => "midnight",
        PresentationTheme::Sage => "sage",
    }
}

fn slide_document(background: &str, shapes: &str, index: usize, total: usize) -> String {
    let page = text_box(
        99,
        "页码",
        10_850_000,
        6_130_000,
        620_000,
        270_000,
        &format!("{index:02} / {total:02}"),
        850,
        "8E8E93",
        false,
        "r",
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="{background}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}{page}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    )
}

#[allow(clippy::too_many_arguments)]
fn text_box(
    id: usize,
    name: &str,
    x: usize,
    y: usize,
    cx: usize,
    cy: usize,
    text: &str,
    size: usize,
    color: &str,
    bold: bool,
    align: &str,
) -> String {
    let bold = if bold { " b=\"1\"" } else { "" };
    let paragraphs = text
        .split('\n')
        .map(|line| {
            format!(
                r#"<a:p><a:pPr algn="{align}"/><a:r><a:rPr lang="zh-CN" sz="{size}"{bold}><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN" sz="{size}"/></a:p>"#,
                xml_escape(line)
            )
        })
        .collect::<String>();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="ctr"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#,
        xml_escape(name)
    )
}

#[allow(clippy::too_many_arguments)]
fn bullet_box(
    id: usize,
    name: &str,
    x: usize,
    y: usize,
    cx: usize,
    cy: usize,
    items: &[String],
    size: usize,
    color: &str,
) -> String {
    let paragraphs = items
        .iter()
        .take(5)
        .map(|item| {
            format!(
                r#"<a:p><a:pPr marL="280000" indent="-210000"><a:buChar char="•"/></a:pPr><a:r><a:rPr lang="zh-CN" sz="{size}"><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN" sz="{size}"/></a:p>"#,
                xml_escape(item)
            )
        })
        .collect::<String>();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#,
        xml_escape(name)
    )
}

#[allow(clippy::too_many_arguments)]
fn shape(
    id: usize,
    name: &str,
    geometry: &str,
    x: usize,
    y: usize,
    cx: usize,
    cy: usize,
    fill: &str,
    line: &str,
) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="{geometry}"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{fill}"/></a:solidFill><a:ln><a:solidFill><a:srgbClr val="{line}"/></a:solidFill></a:ln></p:spPr></p:sp>"#,
        xml_escape(name)
    )
}

fn write_part(
    archive: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    name: &str,
    content: &str,
) -> Result<(), BrainError> {
    archive
        .start_file(name, options)
        .map_err(|error| BrainError::Internal(format!("PPTX 部件创建失败: {error}")))?;
    archive.write_all(content.as_bytes())?;
    Ok(())
}

fn content_types(slide_count: usize) -> String {
    let slides = (1..=slide_count)
        .map(|index| format!(r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>{slides}</Types>"#
    )
}

fn presentation_xml(slide_count: usize) -> String {
    let slides = (1..=slide_count)
        .map(|index| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 255 + index, index + 1))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{slides}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000" type="screen16x9"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle/></p:presentation>"#
    )
}

fn presentation_relationships(slide_count: usize) -> String {
    let slides = (1..=slide_count)
        .map(|index| format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{index}.xml"/>"#, index + 1))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>{slides}</Relationships>"#
    )
}

fn app_properties(slide_count: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ObsidianBrain</Application><PresentationFormat>宽屏</PresentationFormat><Slides>{slide_count}</Slides><Notes>0</Notes><HiddenSlides>0</HiddenSlides><MMClips>0</MMClips><ScaleCrop>false</ScaleCrop><AppVersion>2.0</AppVersion></Properties>"#
    )
}

fn core_properties(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>ObsidianBrain</dc:creator><cp:lastModifiedBy>ObsidianBrain</cp:lastModifiedBy></cp:coreProperties>"#,
        xml_escape(title)
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;
const SLIDE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#;
const MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;
const LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;
const LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="结构化演示文稿"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;
const MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1="5A4AE3" accent2="D85C41" accent3="2C6B4F" accent4="C9822D" accent5="AF52DE" accent6="FF375F" bg1="F4F0E8" bg2="FFFCF6" folHlink="AF52DE" hlink="007AFF" tx1="1D1D1F" tx2="68645F"/><p:sldLayoutIdLst><p:sldLayoutId id="1" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#;
const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ObsidianBrain Editorial"><a:themeElements><a:clrScheme name="ObsidianBrain"><a:dk1><a:srgbClr val="1D1D1F"/></a:dk1><a:lt1><a:srgbClr val="F4F0E8"/></a:lt1><a:dk2><a:srgbClr val="3A3A3C"/></a:dk2><a:lt2><a:srgbClr val="FFFCF6"/></a:lt2><a:accent1><a:srgbClr val="5A4AE3"/></a:accent1><a:accent2><a:srgbClr val="D85C41"/></a:accent2><a:accent3><a:srgbClr val="2C6B4F"/></a:accent3><a:accent4><a:srgbClr val="C9822D"/></a:accent4><a:accent5><a:srgbClr val="AF52DE"/></a:accent5><a:accent6><a:srgbClr val="FF375F"/></a:accent6><a:hlink><a:srgbClr val="007AFF"/></a:hlink><a:folHlink><a:srgbClr val="AF52DE"/></a:folHlink></a:clrScheme><a:fontScheme name="ObsidianBrain"><a:majorFont><a:latin typeface="Aptos Display"/><a:ea typeface="PingFang SC"/><a:cs typeface="Arial"/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/><a:ea typeface="PingFang SC"/><a:cs typeface="Arial"/></a:minorFont></a:fontScheme><a:fmtScheme name="ObsidianBrain"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> PresentationSpec {
        serde_json::from_value(serde_json::json!({
            "schema_version": "1.0",
            "title": "分层架构为什么能保持演进速度",
            "subtitle": "《架构之美》专题研究",
            "audience": "需要做架构取舍的研发团队",
            "core_message": "分层的价值不在层数，而在稳定依赖方向。",
            "theme": "editorial",
            "slides": [
                {"layout":"statement","eyebrow":"核心判断","title":"分层首先是依赖约束","takeaway":"职责划分只是表象，可预测的依赖方向才是架构收益。","body":["界面层依赖服务层，服务层依赖存储抽象。"],"citations":["S1"]},
                {"layout":"split","eyebrow":"取舍","title":"边界稳定与局部效率需要同时衡量","takeaway":"过度抽象和无边界调用都会伤害演进速度。","left":{"label":"稳定性","title":"稳定边界","points":["变更影响范围可预测"]},"right":{"label":"效率","title":"避免仪式化抽象","points":["小型局部逻辑不必强制分层"]},"citations":["S1"]},
                {"layout":"process","eyebrow":"实施","title":"从职责到验证的三步闭环","takeaway":"先定边界，再约束依赖，最后用变更验证。","steps":[{"title":"划分","detail":"识别变化速率不同的职责"},{"title":"约束","detail":"保持单向依赖和少量入口"},{"title":"验证","detail":"以真实变更检查边界质量"}],"citations":["S1"]},
                {"layout":"summary","eyebrow":"行动","title":"把分层变成可检查的工程约束","takeaway":"好架构的判断标准是下一次变更是否更容易。","body":["明确每层负责的变化。","检查跨层调用。","用变更成本复盘边界。"],"citations":[]}
            ]
        })).unwrap()
    }

    #[test]
    fn test_parse_presentation_spec_normalizes_citations_and_rejects_trailing_text() {
        let mut value = serde_json::to_value(sample_spec()).unwrap();
        value["slides"][0]["citations"] = serde_json::json!(["[s1]"]);
        let parsed = parse_presentation_spec(&value.to_string(), 1).unwrap();
        assert_eq!(parsed.slides[0].citations, vec!["S1"]);
        let error = parse_presentation_spec(&format!("{}\n完成", value), 1).unwrap_err();
        assert!(error.to_string().contains("多余内容"));
    }

    #[test]
    fn test_validate_presentation_spec_rejects_repetitive_plan() {
        let mut spec = sample_spec();
        spec.slides.push(spec.slides[0].clone());
        spec.slides.last_mut().unwrap().title = "额外内容页".into();
        for slide in &mut spec.slides {
            slide.layout = PresentationLayout::Statement;
        }
        let error = validate_presentation_spec(&spec, 1).unwrap_err();
        assert!(error.to_string().contains("至少需要 3 种布局"));
    }

    #[test]
    fn test_validate_presentation_spec_rejects_repeated_titles_and_takeaways() {
        let mut spec = sample_spec();
        spec.slides[1].title = format!("{}。", spec.slides[0].title);
        let duplicate = validate_presentation_spec(&spec, 1).unwrap_err();
        assert!(duplicate.to_string().contains("标题重复"));

        let mut spec = sample_spec();
        spec.slides[1].takeaway = format!("{}。", spec.slides[1].title);
        let repeated = validate_presentation_spec(&spec, 1).unwrap_err();
        assert!(repeated.to_string().contains("重复本页标题"));
    }

    #[test]
    fn test_validate_presentation_spec_allows_direct_topic_title() {
        let mut spec = sample_spec();
        spec.slides[2].title = "分层实施流程".into();
        assert!(validate_presentation_spec(&spec, 1).is_ok());
    }

    #[test]
    fn test_parse_presentation_spec_accepts_bounded_chart_and_relationship() {
        let value = serde_json::json!({
            "schema_version": "1.0",
            "title": "反馈为什么影响学习速度",
            "subtitle": "《设计心理学》专题研究",
            "audience": "产品设计团队",
            "core_message": "反馈越接近动作，用户越容易建立正确预期。",
            "theme": "editorial",
            "slides": [
                {"layout":"statement","eyebrow":"判断","title":"即时反馈降低操作的不确定性","takeaway":"用户需要在动作之后立刻确认系统状态。","body":["延迟会让用户重复操作。"],"citations":["S1"]},
                {"layout":"chart","eyebrow":"数据","title":"等待时间增加会降低完成率","takeaway":"报告中的对照数据呈现出一致下降趋势。","chart":{"unit":"%","categories":["即时","短暂等待","明显等待"],"values":[92,78,51],"highlight_index":2},"citations":["S1"]},
                {"layout":"relationship","eyebrow":"关系","title":"反馈同时连接动作、状态与下一步","takeaway":"反馈把一次操作转化为可理解的状态变化。","relationship":{"center":{"title":"反馈","detail":"动作后的系统回应"},"related":[{"relation":"确认","title":"用户动作","detail":"系统已收到操作"},{"relation":"解释","title":"当前状态","detail":"说明结果或等待原因"},{"relation":"引导","title":"下一步","detail":"给出继续操作的依据"}]},"citations":["S1"]},
                {"layout":"summary","eyebrow":"收束","title":"反馈规则需要进入交互验收","takeaway":"每个关键动作都应有及时、明确且可行动的回应。","body":["检查反馈时机。","说明当前状态。"],"citations":[]}
            ]
        });

        let spec = parse_presentation_spec(&value.to_string(), 1).expect("rich presentation");
        assert_eq!(spec.slides.len(), 4);
        assert_eq!(layout_name(spec.slides[1].layout), "chart");
        assert_eq!(layout_name(spec.slides[2].layout), "relationship");
        let dir = tempfile::tempdir().expect("tempdir");
        let output = dir.path().join("rich-layouts.pptx");
        render_pptx(&spec, &output).expect("render rich layouts");
        let validation = validate_pptx(&output).expect("validate rich layouts");
        assert_eq!(validation.slide_count, 5);
        assert_eq!(validation.layout_count, 5);
        if let Ok(keep_path) = std::env::var("OBRAIN_KEEP_RICH_TEST_PPTX") {
            std::fs::copy(output, keep_path).unwrap();
        }
    }

    #[test]
    fn test_presentation_chart_rejects_unbounded_or_fabricated_shape() {
        let mut value = serde_json::to_value(sample_spec()).unwrap();
        value["slides"][0] = serde_json::json!({
            "layout": "chart",
            "eyebrow": "数据",
            "title": "错误的数据页",
            "takeaway": "数据必须保持受限。",
            "chart": {
                "unit": "%",
                "categories": ["A", "B"],
                "values": [10, -3],
                "highlight_index": 1
            },
            "citations": ["S1"]
        });

        let error = parse_presentation_spec(&value.to_string(), 1).unwrap_err();
        assert!(error.to_string().contains("不能为负数"));
    }

    #[test]
    fn test_render_pptx_writes_multi_layout_editable_office_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output = dir.path().join("研究成果.pptx");
        render_pptx(&sample_spec(), &output).expect("render");
        let validation = validate_pptx(&output).expect("validate");
        assert_eq!(validation.slide_count, 5);
        assert!(validation.layout_count >= 4);
        assert!(validation.message.contains("可编辑"));
        assert!(std::fs::metadata(&output).unwrap().len() > 4_000);
        if let Ok(keep_path) = std::env::var("OBRAIN_KEEP_TEST_PPTX") {
            std::fs::copy(output, keep_path).unwrap();
        }
    }

    #[test]
    fn test_presentation_quality_report_exposes_auditable_checks() {
        let spec = sample_spec();
        let plan = validate_presentation_spec(&spec, 1).expect("plan validation");
        let dir = tempfile::tempdir().expect("tempdir");
        let output = dir.path().join("quality-report.pptx");
        render_pptx(&spec, &output).expect("render");
        let package = validate_pptx(&output).expect("package validation");

        let report = build_presentation_quality_report(&spec, &plan, &package);

        assert_eq!(report.slide_count, 5);
        assert_eq!(report.content_slide_count, 4);
        assert_eq!(report.cited_slide_count, 3);
        assert_eq!(report.citation_coverage_percent, 75);
        assert_eq!(report.layout_distribution.get("statement"), Some(&1));
        assert!(report.package_part_count >= 10);
        assert!(report.checks.iter().all(|check| check.passed));
        assert!(report.summary.contains("引用覆盖 75%"));
        serde_json::to_value(&report).expect("serializable report");
    }
}
