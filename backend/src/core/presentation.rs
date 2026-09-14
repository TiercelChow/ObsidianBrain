use std::fs::File;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::BrainError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PresentationSpec {
    pub title: String,
    pub subtitle: String,
    pub slides: Vec<PresentationSlide>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PresentationSlide {
    pub title: String,
    pub bullets: Vec<String>,
    pub citations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PresentationValidation {
    pub slide_count: usize,
    pub message: String,
}

pub fn spec_from_report(
    title: &str,
    book_name: &str,
    report: &str,
    citations: &[String],
) -> PresentationSpec {
    let mut slides = Vec::new();
    let mut current_title = "研究结论".to_string();
    let mut current_bullets = Vec::new();

    let flush = |slides: &mut Vec<PresentationSlide>, heading: &str, bullets: &mut Vec<String>| {
        if bullets.is_empty() || slides.len() >= 8 {
            return;
        }
        slides.push(PresentationSlide {
            title: truncate_text(heading, 60),
            bullets: bullets.drain(..).take(6).collect(),
            citations: citations.iter().take(3).cloned().collect(),
        });
    };

    for raw_line in report.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(heading) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
        {
            flush(&mut slides, &current_title, &mut current_bullets);
            current_title = heading.trim().to_string();
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let bullet = line.trim_start_matches(['-', '*', '•']).trim();
        if !bullet.is_empty() {
            current_bullets.push(truncate_text(bullet, 180));
        }
        if current_bullets.len() >= 6 {
            flush(&mut slides, &current_title, &mut current_bullets);
            current_title = "研究结论（续）".to_string();
        }
    }
    flush(&mut slides, &current_title, &mut current_bullets);
    if slides.is_empty() {
        slides.push(PresentationSlide {
            title: "核心结论".to_string(),
            bullets: vec![truncate_text(report.trim(), 180)],
            citations: citations.iter().take(3).cloned().collect(),
        });
    }
    slides.push(PresentationSlide {
        title: "总结与下一步".to_string(),
        bullets: vec![
            "回到原书核对关键论断与适用条件。".to_string(),
            "继续在 Wiki 中补充关系、冲突和待解问题。".to_string(),
        ],
        citations: citations.iter().take(3).cloned().collect(),
    });
    PresentationSpec {
        title: truncate_text(title, 80),
        subtitle: format!(
            "《{}》知识研究 · 由 ObsidianBrain 生成",
            truncate_text(book_name, 60)
        ),
        slides,
    }
}

pub fn render_pptx(spec: &PresentationSpec, output: &Path) -> Result<(), BrainError> {
    if spec.title.trim().is_empty() || spec.slides.is_empty() {
        return Err(BrainError::KnowledgeValidation(
            "演示文稿必须包含标题和至少一页内容".to_string(),
        ));
    }
    if spec.slides.len() > 30 {
        return Err(BrainError::KnowledgeValidation(
            "单个演示文稿不能超过 30 页".to_string(),
        ));
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staging = output.with_extension("pptx.part");
    let file = File::create(&staging)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let slide_count = spec.slides.len() + 1;

    write_part(
        &mut archive,
        options,
        "[Content_Types].xml",
        &content_types(slide_count),
    )?;
    write_part(&mut archive, options, "_rels/.rels", ROOT_RELS)?;
    write_part(
        &mut archive,
        options,
        "docProps/app.xml",
        &app_properties(slide_count),
    )?;
    write_part(
        &mut archive,
        options,
        "docProps/core.xml",
        &core_properties(&spec.title),
    )?;
    write_part(
        &mut archive,
        options,
        "ppt/presentation.xml",
        &presentation_xml(slide_count),
    )?;
    write_part(
        &mut archive,
        options,
        "ppt/_rels/presentation.xml.rels",
        &presentation_relationships(slide_count),
    )?;
    write_part(&mut archive, options, "ppt/theme/theme1.xml", THEME_XML)?;
    write_part(
        &mut archive,
        options,
        "ppt/slideMasters/slideMaster1.xml",
        MASTER_XML,
    )?;
    write_part(
        &mut archive,
        options,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        MASTER_RELS,
    )?;
    write_part(
        &mut archive,
        options,
        "ppt/slideLayouts/slideLayout1.xml",
        LAYOUT_XML,
    )?;
    write_part(
        &mut archive,
        options,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        LAYOUT_RELS,
    )?;

    let cover = PresentationSlide {
        title: spec.title.clone(),
        bullets: vec![spec.subtitle.clone()],
        citations: Vec::new(),
    };
    for (index, slide) in std::iter::once(&cover)
        .chain(spec.slides.iter())
        .enumerate()
    {
        let number = index + 1;
        write_part(
            &mut archive,
            options,
            &format!("ppt/slides/slide{number}.xml"),
            &slide_xml(slide, number, slide_count),
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
        "ppt/slides/slide1.xml",
    ] {
        archive
            .by_name(required)
            .map_err(|_| BrainError::Internal(format!("PPTX 缺少必要部件: {required}")))?;
    }
    let mut slide_count = 0;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| BrainError::Internal(format!("PPTX 部件读取失败: {error}")))?;
        let name = file.name();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_count += 1;
        }
    }
    if slide_count == 0 {
        return Err(BrainError::Internal("PPTX 没有可显示页面".to_string()));
    }
    Ok(PresentationValidation {
        slide_count,
        message: format!(
            "文件结构完整，共 {slide_count} 页；当前环境未执行 Office/Keynote 视觉预览"
        ),
    })
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

fn slide_xml(slide: &PresentationSlide, index: usize, total: usize) -> String {
    let title = xml_escape(&slide.title);
    let bullets = slide
        .bullets
        .iter()
        .take(6)
        .map(|bullet| format!(r#"<a:p><a:pPr marL="342900" indent="-285750"><a:buChar char="•"/></a:pPr><a:r><a:rPr lang="zh-CN" sz="2200"/><a:t>{}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p>"#, xml_escape(bullet)))
        .collect::<String>();
    let citations = xml_escape(&slide.citations.join(" · "));
    let shapes = format!(
        "{}{}{}",
        text_box(2, "标题", 760000, 520000, 10600000, 900000, &title, 3400, "1D1D1F", true,),
        body_box(&bullets),
        text_box(5, "来源", 760000, 6120000, 10000000, 300000, &citations, 850, "6E6E73", false,)
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="F7F5F0"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}<p:sp><p:nvSpPr><p:cNvPr id="4" name="页码"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="11000000" y="6300000"/><a:ext cx="700000" cy="300000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="900"><a:solidFill><a:srgbClr val="8E8E93"/></a:solidFill></a:rPr><a:t>{index}/{total}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    )
}

fn body_box(paragraphs: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="正文"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="780000" y="1650000"/><a:ext cx="10400000" cy="4100000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
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
) -> String {
    let bold = if bold { " b=\"1\"" } else { "" };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="ctr"/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="{size}"{bold}><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></a:rPr><a:t>{text}</a:t></a:r><a:endParaRPr lang="zh-CN"/></a:p></p:txBody></p:sp>"#
    )
}

fn app_properties(slide_count: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>ObsidianBrain</Application><PresentationFormat>宽屏</PresentationFormat><Slides>{slide_count}</Slides><Notes>0</Notes><HiddenSlides>0</HiddenSlides><MMClips>0</MMClips><ScaleCrop>false</ScaleCrop><AppVersion>1.0</AppVersion></Properties>"#
    )
}

fn core_properties(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>ObsidianBrain</dc:creator><cp:lastModifiedBy>ObsidianBrain</cp:lastModifiedBy></cp:coreProperties>"#,
        xml_escape(title)
    )
}

fn truncate_text(value: &str, limit: usize) -> String {
    let mut result = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        result.push('…');
    }
    result
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
const LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="空白"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;
const MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap accent1="5B5BD6" accent2="30B0C7" accent3="34C759" accent4="FF9F0A" accent5="AF52DE" accent6="FF375F" bg1="F7F5F0" bg2="E5E5EA" folHlink="AF52DE" hlink="007AFF" tx1="1D1D1F" tx2="6E6E73"/><p:sldLayoutIdLst><p:sldLayoutId id="1" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#;
const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ObsidianBrain"><a:themeElements><a:clrScheme name="ObsidianBrain"><a:dk1><a:srgbClr val="1D1D1F"/></a:dk1><a:lt1><a:srgbClr val="F7F5F0"/></a:lt1><a:dk2><a:srgbClr val="3A3A3C"/></a:dk2><a:lt2><a:srgbClr val="E5E5EA"/></a:lt2><a:accent1><a:srgbClr val="5B5BD6"/></a:accent1><a:accent2><a:srgbClr val="30B0C7"/></a:accent2><a:accent3><a:srgbClr val="34C759"/></a:accent3><a:accent4><a:srgbClr val="FF9F0A"/></a:accent4><a:accent5><a:srgbClr val="AF52DE"/></a:accent5><a:accent6><a:srgbClr val="FF375F"/></a:accent6><a:hlink><a:srgbClr val="007AFF"/></a:hlink><a:folHlink><a:srgbClr val="AF52DE"/></a:folHlink></a:clrScheme><a:fontScheme name="ObsidianBrain"><a:majorFont><a:latin typeface="Aptos Display"/><a:ea typeface="PingFang SC"/><a:cs typeface="Arial"/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/><a:ea typeface="PingFang SC"/><a:cs typeface="Arial"/></a:minorFont></a:fontScheme><a:fmtScheme name="ObsidianBrain"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_pptx_writes_valid_editable_office_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output = dir.path().join("研究成果.pptx");
        let spec = spec_from_report(
            "分层架构分享",
            "架构之美",
            "## 核心概念\n- 界面层负责交互\n- 服务层负责业务\n## 注意事项\n依赖方向必须稳定。",
            &["chapter-1.md · 分层架构".to_string()],
        );

        render_pptx(&spec, &output).expect("render");
        let validation = validate_pptx(&output).expect("validate");

        assert_eq!(validation.slide_count, 4);
        assert!(std::fs::metadata(output).unwrap().len() > 2_000);
        if let Ok(keep_path) = std::env::var("OBRAIN_KEEP_TEST_PPTX") {
            std::fs::copy(dir.path().join("研究成果.pptx"), keep_path).unwrap();
        }
    }
}
