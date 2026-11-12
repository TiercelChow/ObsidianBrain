//! Bounded, chapter-at-a-time EPUB 2/3 reader; never extracts archive paths.
use crate::error::BrainError;
use roxmltree::{Document, Node};
use serde::Serialize;
use std::{collections::HashMap, fs::File, io::Read, path::Path};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
pub struct EpubChapter {
    pub href: String,
    pub title: String,
}
#[derive(Debug, Serialize)]
pub struct EpubBook {
    pub title: String,
    pub chapters: Vec<EpubChapter>,
    pub toc: Vec<EpubChapter>,
}

fn invalid(s: impl std::fmt::Display) -> BrainError {
    BrainError::KnowledgeValidation(format!("EPUB：{s}"))
}
pub fn resolve(base: &str, href: &str) -> Result<String, BrainError> {
    let href = urlencoding::decode(href).map_err(invalid)?;
    if href.starts_with('/') || href.contains(['\\', '\0', ':', '?']) {
        return Err(invalid("资源路径不合法"));
    }
    let (path, fragment) = href.split_once('#').unwrap_or((&href, ""));
    let mut parts: Vec<&str> = base
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').collect())
        .unwrap_or_default();
    if !path.is_empty() {
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(invalid("资源越过书籍根目录"));
                    }
                }
                _ => parts.push(part),
            }
        }
    } else {
        parts = base.split('/').collect();
    }
    let mut result = parts.join("/");
    if !fragment.is_empty() {
        result.push('#');
        result.push_str(fragment);
    }
    Ok(result)
}
fn open(path: &Path) -> Result<ZipArchive<File>, BrainError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > 512 * 1024 * 1024 {
        return Err(invalid("文件超过 512 MiB"));
    }
    let mut archive = ZipArchive::new(file).map_err(invalid)?;
    if archive.len() > 20_000 {
        return Err(invalid("资源数量超过上限"));
    }
    let mut seen = std::collections::HashSet::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(invalid)?;
        let name = entry.name();
        if name.starts_with('/')
            || name.contains(['\\', '\0'])
            || name.split('/').any(|p| p == ".." || p == ".")
            || !seen.insert(name.to_string())
        {
            return Err(invalid("压缩包存在不安全或重复的资源路径"));
        }
    }
    Ok(archive)
}
fn read(archive: &mut ZipArchive<File>, entry: &str, limit: u64) -> Result<Vec<u8>, BrainError> {
    let file = archive
        .by_name(entry)
        .map_err(|e| invalid(format!("无法读取 {entry}（文件损坏、加密或资源缺失）：{e}")))?;
    if file.size() > limit || file.size() > file.compressed_size().max(1) * 1000 {
        return Err(invalid("资源大小或压缩比超过安全上限"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("资源过大"));
    }
    Ok(bytes)
}
fn xml(bytes: Vec<u8>) -> Result<String, BrainError> {
    let source = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if bytes.len() & 1 != 0 {
            return Err(invalid("UTF-16 XML 长度无效"));
        }
        let units = bytes[2..]
            .chunks(2)
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect::<Vec<_>>();
        String::from_utf16(&units).map_err(|_| invalid("UTF-16 XML 编码无效"))?
    } else {
        String::from_utf8(bytes).map_err(|_| invalid("XML 必须使用 UTF-8 或 UTF-16 编码"))?
    };
    // EPUB 2 publishers often use XHTML named entities from an external DTD.
    // Resolve the fixed HTML entity vocabulary locally, never fetch a DTD.
    let mut out = String::with_capacity(source.len());
    let mut rest = source.as_str();
    while !rest.is_empty() {
        if let Some((start, end)) = [("<![CDATA[", "]]>"), ("<!--", "-->")]
            .into_iter()
            .find(|(start, _)| rest.starts_with(start))
        {
            if let Some(stop) = rest[start.len()..].find(end) {
                let len = start.len() + stop + end.len();
                out.push_str(&rest[..len]);
                rest = &rest[len..];
                continue;
            }
        }
        if rest.starts_with('&') {
            if let Some(stop) = rest.as_bytes().iter().take(40).position(|b| *b == b';') {
                let entity = &rest[..=stop];
                let name = &rest[1..stop];
                if !name.starts_with('#') && !["amp", "lt", "gt", "quot", "apos"].contains(&name) {
                    let decoded = html_escape::decode_html_entities(entity);
                    if decoded != entity {
                        for ch in decoded.chars() {
                            out.push_str(&format!("&#{};", ch as u32));
                        }
                        rest = &rest[stop + 1..];
                        continue;
                    }
                }
            }
        }
        let ch = rest.chars().next().ok_or_else(|| invalid("XML 字符无效"))?;
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    Ok(out)
}
fn parse(s: &str) -> Result<Document<'_>, BrainError> {
    Document::parse_with_options(
        s,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 300_000,
            entity_resolver: None,
        },
    )
    .map_err(|e| invalid(format!("XML 格式错误：{e}")))
}
fn text(n: Node<'_, '_>) -> String {
    n.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn metadata(path: &Path) -> Result<EpubBook, BrainError> {
    let mut archive = open(path)?;
    let container = xml(read(
        &mut archive,
        "META-INF/container.xml",
        2 * 1024 * 1024,
    )?)?;
    let container = parse(&container)?;
    let opf = container
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| invalid("缺少 OPF 路径"))?;
    let opf = resolve("", opf)?;
    let package = xml(read(&mut archive, &opf, 2 * 1024 * 1024)?)?;
    let package = parse(&package)?;
    let title = package
        .descendants()
        .find(|n| n.has_tag_name("title"))
        .map(text)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "EPUB".into());
    let mut manifest = HashMap::new();
    let mut nav = None;
    for n in package.descendants().filter(|n| n.has_tag_name("item")) {
        if let (Some(id), Some(href)) = (n.attribute("id"), n.attribute("href")) {
            let href = resolve(&opf, href)?;
            if n.attribute("properties")
                .unwrap_or("")
                .split_whitespace()
                .any(|p| p == "nav")
            {
                nav = Some(href.clone());
            }
            manifest.insert(
                id.to_string(),
                (href, n.attribute("media-type").unwrap_or("").to_string()),
            );
        }
    }
    let spine = package
        .descendants()
        .find(|n| n.has_tag_name("spine"))
        .ok_or_else(|| invalid("缺少阅读顺序"))?;
    let mut chapters = Vec::new();
    for n in spine.children().filter(|n| n.has_tag_name("itemref")) {
        let (href, mime) = manifest
            .get(n.attribute("idref").unwrap_or(""))
            .ok_or_else(|| invalid("章节引用不存在"))?;
        if mime != "application/xhtml+xml" && mime != "text/html" {
            return Err(invalid("章节不是 XHTML/HTML，暂不支持此格式"));
        }
        chapters.push(EpubChapter {
            href: href.clone(),
            title: format!("第 {} 章", chapters.len() + 1),
        });
    }
    if chapters.is_empty() || chapters.len() > 5000 {
        return Err(invalid("书中没有可读章节或章节过多"));
    }
    let mut toc = Vec::new();
    if let Some(nav) = nav {
        let source = xml(read(&mut archive, &nav, 2 * 1024 * 1024)?)?;
        let doc = parse(&source)?;
        if let Some(section) = doc
            .descendants()
            .filter(|n| n.has_tag_name("nav"))
            .find(|n| {
                n.attributes()
                    .any(|a| a.name() == "type" && a.value().split_whitespace().any(|v| v == "toc"))
            })
            .or_else(|| doc.descendants().find(|n| n.has_tag_name("nav")))
        {
            for a in section.descendants().filter(|n| n.has_tag_name("a")) {
                if let Some(href) = a.attribute("href") {
                    toc.push(EpubChapter {
                        href: resolve(&nav, href)?,
                        title: text(a),
                    });
                }
            }
        }
    } else if let Some((ncx, _)) = spine.attribute("toc").and_then(|id| manifest.get(id)) {
        let source = xml(read(&mut archive, ncx, 2 * 1024 * 1024)?)?;
        let doc = parse(&source)?;
        for n in doc.descendants().filter(|n| n.has_tag_name("navPoint")) {
            if let Some(href) = n
                .children()
                .find(|n| n.has_tag_name("content"))
                .and_then(|n| n.attribute("src"))
            {
                toc.push(EpubChapter {
                    href: resolve(ncx, href)?,
                    title: n
                        .children()
                        .find(|n| n.has_tag_name("navLabel"))
                        .map(text)
                        .unwrap_or_default(),
                });
            }
        }
    }
    toc.retain(|item| {
        chapters
            .iter()
            .any(|chapter| chapter.href.split('#').next() == item.href.split('#').next())
    });
    for chapter in &mut chapters {
        if let Some(item) = toc
            .iter()
            .find(|t| t.href.split('#').next() == Some(chapter.href.as_str()))
        {
            chapter.title = item.title.clone();
        }
    }
    if toc.is_empty() {
        toc = chapters.clone();
    }
    Ok(EpubBook {
        title,
        chapters,
        toc,
    })
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn safe_url(value: &str) -> bool {
    let s = value.trim();
    !s.chars().any(char::is_control)
        && !s.contains(':')
        && !s.starts_with("//")
        && !s.contains('\\')
}
const MAX_CHAPTER_DEPTH: usize = 256;

fn serialize(node: Node<'_, '_>, out: &mut String, depth: usize) -> Result<(), BrainError> {
    // The XML parser limits the number of nodes, not their nesting depth.
    if depth > MAX_CHAPTER_DEPTH {
        return Err(invalid("章节 XML 嵌套过深"));
    }
    if node.is_text() {
        if node
            .parent_element()
            .is_some_and(|p| p.has_tag_name("style"))
        {
            out.push_str(&node.text().unwrap_or("").replace('<', "\\3c "));
        } else {
            out.push_str(&escape(node.text().unwrap_or("")));
        }
        return Ok(());
    }
    if !node.is_element() {
        for child in node.children() {
            serialize(child, out, depth + 1)?;
        }
        return Ok(());
    }
    let name = node.tag_name().name();
    if [
        "script",
        "iframe",
        "frame",
        "frameset",
        "object",
        "embed",
        "form",
        "input",
        "button",
        "textarea",
        "select",
        "base",
        "meta",
        "foreignObject",
    ]
    .iter()
    .any(|v| name.eq_ignore_ascii_case(v))
    {
        return Ok(());
    }
    out.push('<');
    out.push_str(name);
    if ["svg", "math"].contains(&name) {
        if let Some(ns) = node.tag_name().namespace() {
            out.push_str(" xmlns=\"");
            out.push_str(&escape(ns));
            out.push('"');
        }
    }
    for a in node.attributes() {
        let attr = a.name().to_ascii_lowercase();
        if attr.starts_with("on")
            || [
                "srcdoc",
                "action",
                "formaction",
                "target",
                "download",
                "base",
                "srcset",
                "ping",
            ]
            .contains(&attr.as_str())
        {
            continue;
        }
        if ["href", "src", "poster"].contains(&attr.as_str()) && !safe_url(a.value()) {
            continue;
        }
        out.push(' ');
        out.push_str(a.name());
        out.push_str("=\"");
        out.push_str(&escape(a.value()));
        out.push('"');
    }
    out.push('>');
    for child in node.children() {
        serialize(child, out, depth + 1)?;
    }
    if !["br", "hr", "img", "link", "source", "wbr", "area", "col"].contains(&name) {
        out.push_str("</");
        out.push_str(name);
        out.push('>');
    }
    Ok(())
}
pub fn resource(path: &Path, entry: &str) -> Result<(Vec<u8>, String), BrainError> {
    if entry.is_empty()
        || entry.contains(['\\', '\0'])
        || entry.starts_with('/')
        || entry.split('/').any(|p| p == ".." || p == ".")
    {
        return Err(invalid("资源路径不合法"));
    }
    let mut archive = open(path)?;
    let ext = Path::new(entry)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let html = ["xhtml", "html", "htm"].contains(&ext.as_str());
    let bytes = read(
        &mut archive,
        entry,
        if html {
            8 * 1024 * 1024
        } else {
            32 * 1024 * 1024
        },
    )?;
    if html {
        let source = xml(bytes)?;
        let document = parse(&source)?;
        let mut clean = String::from("<!DOCTYPE html>");
        serialize(document.root(), &mut clean, 0)?;
        Ok((clean.into_bytes(), "text/html; charset=utf-8".into()))
    } else {
        let mime = match ext.as_str() {
            "css" => "text/css",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            _ => return Err(invalid("不支持此类资源")),
        };
        // SVG is passive when loaded as an image; direct navigation remains
        // sandboxed and script-free by the response CSP.
        Ok((bytes, mime.into()))
    }
}

#[cfg(test)]
pub(crate) mod tests;
