use super::*;

#[derive(Serialize)]
pub(crate) struct PresentationEditableField {
    path: String,
    label: String,
    text: String,
}

#[derive(Serialize)]
pub(crate) struct PresentationPageEditingContent {
    fields: Vec<PresentationEditableField>,
    fixed_text: Vec<String>,
    used_characters: usize,
    character_limit: usize,
}

pub(crate) fn page_editing_content(slide: &PresentationSlide) -> PresentationPageEditingContent {
    let mut fields = Vec::new();
    let mut field = |path: String, label: String, text: &str| {
        fields.push(PresentationEditableField {
            path,
            label,
            text: text.to_owned(),
        });
    };
    field("/eyebrow".into(), "眉题".into(), &slide.eyebrow);
    field("/title".into(), "标题".into(), &slide.title);
    field("/takeaway".into(), "本页要点".into(), &slide.takeaway);
    for (index, text) in slide.body.iter().enumerate() {
        field(
            format!("/body/{index}"),
            format!("要点 {}", index + 1),
            text,
        );
    }
    for (name, label, panel) in [
        ("left", "左栏", slide.left.as_ref()),
        ("right", "右栏", slide.right.as_ref()),
    ] {
        if let Some(panel) = panel {
            field(
                format!("/{name}/label"),
                format!("{label}标签"),
                &panel.label,
            );
            field(
                format!("/{name}/title"),
                format!("{label}标题"),
                &panel.title,
            );
            for (index, text) in panel.points.iter().enumerate() {
                field(
                    format!("/{name}/points/{index}"),
                    format!("{label}要点 {}", index + 1),
                    text,
                );
            }
        }
    }
    for (index, step) in slide.steps.iter().enumerate() {
        field(
            format!("/steps/{index}/title"),
            format!("第 {} 步标题", index + 1),
            &step.title,
        );
        field(
            format!("/steps/{index}/detail"),
            format!("第 {} 步说明", index + 1),
            &step.detail,
        );
    }
    let mut fixed_text = Vec::new();
    if let Some(metric) = &slide.metric {
        fixed_text.push(metric.value.clone());
        field("/metric/label".into(), "指标名称".into(), &metric.label);
        field("/metric/context".into(), "指标口径".into(), &metric.context);
    }
    if let Some(chart) = &slide.chart {
        fixed_text.extend(
            chart
                .categories
                .iter()
                .zip(&chart.values)
                .map(|(label, value)| {
                    format!("{label}  {} {}", format_chart_value(*value), chart.unit)
                }),
        );
        field("/chart/context".into(), "图表口径".into(), &chart.context);
    }
    if let Some(graph) = &slide.relationship {
        field(
            "/relationship/center/title".into(),
            "中心名称".into(),
            &graph.center.title,
        );
        field(
            "/relationship/center/detail".into(),
            "中心说明".into(),
            &graph.center.detail,
        );
        for (index, node) in graph.related.iter().enumerate() {
            let label = format!("关系 {}", index + 1);
            field(
                format!("/relationship/related/{index}/relation"),
                label.clone(),
                &node.relation,
            );
            field(
                format!("/relationship/related/{index}/title"),
                format!("{label}对象"),
                &node.title,
            );
            field(
                format!("/relationship/related/{index}/detail"),
                format!("{label}说明"),
                &node.detail,
            );
        }
    }
    if !slide.quote.is_empty() {
        fixed_text.push(slide.quote.clone());
    }
    PresentationPageEditingContent {
        fields,
        fixed_text,
        used_characters: slide_character_count(slide),
        character_limit: MAX_SLIDE_CHARACTERS,
    }
}

pub(crate) fn edit_page_text(
    spec: &mut PresentationSpec,
    page: usize,
    changes: &BTreeMap<String, String>,
) -> Result<(), BrainError> {
    let slide = page
        .checked_sub(2)
        .and_then(|index| spec.slides.get_mut(index))
        .ok_or_else(|| validation_error("请选择一张正文或附录页"))?;
    let editing = page_editing_content(slide);
    let mut json = serde_json::to_value(&*slide)
        .map_err(|error| BrainError::Internal(format!("演示页面序列化失败: {error}")))?;
    let mut changed = false;
    for (path, replacement) in changes {
        let value = replacement.trim();
        if value.chars().any(|ch| {
            matches!(ch, '\u{fffe}' | '\u{ffff}')
                || (ch.is_control() && !matches!(ch, '\n' | '\t' | '\r'))
        }) {
            return Err(validation_error("页面文字含有不可显示的字符"));
        }
        let field = editing
            .fields
            .iter()
            .find(|field| &field.path == path)
            .ok_or_else(|| validation_error("只能修改所选页面中的可编辑文字"))?;
        if value == field.text {
            continue;
        }
        let target = json
            .pointer_mut(path)
            .ok_or_else(|| BrainError::Internal("演示页面缺少可编辑字段".into()))?;
        *target = serde_json::Value::String(value.to_owned());
        changed = true;
    }
    if !changed {
        return Err(validation_error("这一页没有文字变更"));
    }
    *slide = serde_json::from_value(json)
        .map_err(|error| BrainError::Internal(format!("演示页面解析失败: {error}")))?;
    Ok(())
}

/// Transfer the package with exactly one slide XML part replaced. Copying the
/// remaining parts preserves older renderer output and other pages byte for byte.
pub(crate) fn render_pptx_page_revision(
    spec: &PresentationSpec,
    original: &Path,
    output: &Path,
    page: usize,
) -> Result<(), BrainError> {
    let slide = page
        .checked_sub(2)
        .and_then(|index| spec.slides.get(index))
        .ok_or_else(|| validation_error("请选择一张正文或附录页"))?;
    let palette = Palette::for_theme(spec.theme);
    let xml = slide_document(
        palette.background,
        &content_slide_shapes(slide, &palette),
        page,
        spec.slides.len() + 1,
    );
    let target = format!("ppt/slides/slide{page}.xml");
    let zip_error = |error| BrainError::Internal(format!("PPTX 页面复制失败: {error}"));
    let mut source = ZipArchive::new(File::open(original)?).map_err(zip_error)?;
    source.by_name(&target).map_err(zip_error)?;
    let mut destination = ZipWriter::new(File::create(output)?);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for index in 0..source.len() {
        let entry = source.by_index(index).map_err(zip_error)?;
        if entry.name() == target {
            write_part(&mut destination, options, &target, &xml)?;
        } else {
            destination.raw_copy_file(entry).map_err(zip_error)?;
        }
    }
    destination.finish().map_err(zip_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> PresentationSpec {
        parse_presentation_spec(
            &serde_json::json!({
                "schema_version":"2.0","title":"可复用的研究报告","subtitle":"逐页修订",
                "audience":"读者","core_message":"保留每页的论点和出处。",
                "slides":[
                    {"layout":"statement","role":"claim","estimated_seconds":30,
                     "eyebrow":"示例","title":"架构职责","takeaway":"分别说明各层的分工。",
                     "body":["界面层负责展示。"],"citations":["S1"]},
                    {"layout":"summary","role":"action","estimated_seconds":40,
                     "eyebrow":"建议","title":"检查边界调用","takeaway":"跨层调用需明确。",
                     "body":["验证读写路径。"],"citations":["S1"]}
                ]
            })
            .to_string(),
            1,
        )
        .unwrap()
    }

    #[test]
    fn test_edit_changes_only_an_existing_text_field_on_the_selected_page() {
        let mut spec = sample_spec();
        let original = spec.clone();
        edit_page_text(
            &mut spec,
            2,
            &BTreeMap::from([("/title".into(), "新的页面标题".into())]),
        )
        .unwrap();
        assert_eq!(spec.slides[0].title, "新的页面标题");
        assert_eq!(spec.slides[0].citations, original.slides[0].citations);
        assert_eq!(&spec.slides[1..], &original.slides[1..]);
        assert_eq!(spec.title, original.title);
        let result = edit_page_text(
            &mut spec,
            2,
            &BTreeMap::from([("/citations/0".into(), "S2".into())]),
        );
        assert!(result.is_err());
        assert!(edit_page_text(
            &mut spec,
            1,
            &BTreeMap::from([("/title".into(), "封面".into())]),
        )
        .is_err());
    }

    #[test]
    fn test_edit_preserves_the_exact_original_xml_for_locked_pages() {
        let dir = tempfile::tempdir().unwrap();
        let original_path = dir.path().join("original.pptx");
        let revised_path = dir.path().join("revised.pptx");
        let mut spec = sample_spec();
        render_pptx(&spec, &original_path).unwrap();
        // Simulate an original produced by a different cover layout.
        spec.title = "其他页面不应受到当前渲染器影响".into();
        edit_page_text(
            &mut spec,
            2,
            &BTreeMap::from([("/title".into(), "修订后的页面标题".into())]),
        )
        .unwrap();
        render_pptx_page_revision(&spec, &original_path, &revised_path, 2).unwrap();

        fn part(path: &Path, name: &str) -> Vec<u8> {
            let mut deck = ZipArchive::new(File::open(path).unwrap()).unwrap();
            let mut xml = Vec::new();
            deck.by_name(name).unwrap().read_to_end(&mut xml).unwrap();
            xml
        }
        for name in [
            "ppt/slides/slide1.xml",
            "ppt/slides/slide3.xml",
            "ppt/presentation.xml",
        ] {
            assert_eq!(part(&original_path, name), part(&revised_path, name));
        }
        assert!(
            !String::from_utf8_lossy(&part(&original_path, "ppt/slides/slide2.xml"))
                .contains("修订后的页面标题")
        );
        assert!(
            String::from_utf8_lossy(&part(&revised_path, "ppt/slides/slide2.xml"))
                .contains("修订后的页面标题")
        );
        assert_eq!(
            validate_pptx(&revised_path).unwrap().slide_count,
            spec.slides.len() + 1
        );
    }
}
