use super::*;
use std::io::Write;
pub(crate) fn book() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.epub");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for (name, body) in [
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OPS/book.opf",
            r#"<package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>测试书</dc:title></metadata><manifest><item id="one" href="chapters/one.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" properties="nav"/></manifest><spine><itemref idref="one"/></spine></package>"#,
        ),
        (
            "OPS/nav.xhtml",
            r#"<html><body><nav><a href="chapters/one.xhtml">开始阅读</a></nav></body></html>"#,
        ),
        (
            "OPS/chapters/one.xhtml",
            r#"<html><head><meta http-equiv="refresh" content="0;url=https://bad"/></head><body onload="bad()"><h1>你好</h1><script>bad()</script><img src="../images/p.png"/><a href="javascript:bad()">链接</a></body></html>"#,
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    (dir, path)
}
#[test]
fn test_epub_metadata_and_safe_chapter() {
    let (_d, path) = book();
    let book = metadata(&path).unwrap();
    assert_eq!(book.title, "测试书");
    assert_eq!(book.chapters[0].title, "开始阅读");
    let (bytes, mime) = resource(&path, &book.chapters[0].href).unwrap();
    let html = String::from_utf8(bytes).unwrap();
    assert!(mime.starts_with("text/html"));
    assert!(html.contains("你好"));
    assert!(html.contains("../images/p.png"));
    for unsafe_content in ["onload", "script", "refresh", "javascript:"] {
        assert!(!html.contains(unsafe_content));
    }
}
#[test]
fn test_epub_path_boundaries_and_corrupt_archive() {
    assert_eq!(
        resolve("OPS/nav.xhtml", "chapters/第一章.xhtml#part").unwrap(),
        "OPS/chapters/第一章.xhtml#part"
    );
    for href in [
        "../../etc/passwd",
        "https://evil",
        "/absolute",
        "..\\outside",
    ] {
        assert!(resolve("OPS/book.opf", href).is_err());
    }
    let (_d, path) = book();
    assert!(resource(&path, "../OPS/book.opf").is_err());
    std::fs::write(&path, b"not zip").unwrap();
    assert!(metadata(&path).is_err());
}

#[test]
fn test_epub_chapter_rejects_excessive_nesting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep.epub");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for (name, depth) in [("normal.xhtml", 64), ("deep.xhtml", 512)] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        write!(
            zip,
            "{}正文{}",
            "<div>".repeat(depth),
            "</div>".repeat(depth)
        )
        .unwrap();
    }
    zip.finish().unwrap();
    let (bytes, _) = resource(&path, "normal.xhtml").unwrap();
    assert!(String::from_utf8(bytes).unwrap().contains("正文"));
    assert!(
        matches!(resource(&path, "deep.xhtml"), Err(error) if error.to_string().contains("嵌套过深"))
    );
}

#[test]
fn test_epub2_doctype_namespaces_entities_and_css_remain_safe() {
    let source=xml(br#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd"><html xmlns="http://www.w3.org/1999/xhtml"><head><style>body &gt; p {color:red}</style></head><body><p>A&nbsp;B &copy; &lt;code&gt;</p></body></html>"#.to_vec()).unwrap();
    let doc = parse(&source).unwrap();
    assert!(doc.descendants().any(|n| n.has_tag_name("p")));
    let mut out = String::new();
    serialize(doc.root(), &mut out, 0).unwrap();
    assert!(out.contains("body > p"));
    assert!(out.contains("A\u{a0}B © &lt;code&gt;"));
    let external = r#"<!DOCTYPE html [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><html><body>&xxe;</body></html>"#;
    assert!(parse(external).is_err());
}

#[test]
fn test_epub2_ncx_and_utf16_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("epub2.epub");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for (name, body) in [
        (
            "META-INF/container.xml",
            r#"<container><rootfile full-path="book.opf"/></container>"#,
        ),
        (
            "book.opf",
            r#"<package><metadata><title>EPUB 2</title></metadata><manifest><item id="one" href="one.xhtml" media-type="application/xhtml+xml"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="ncx"><itemref idref="one"/></spine></package>"#,
        ),
        (
            "toc.ncx",
            r#"<ncx><navMap><navPoint><navLabel><text>NCX 章节</text></navLabel><content src="one.xhtml#section"/></navPoint></navMap></ncx>"#,
        ),
        (
            "one.xhtml",
            r#"<html><body><h1 id="section">EPUB 2</h1></body></html>"#,
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    let book = metadata(&path).unwrap();
    assert_eq!(book.chapters[0].title, "NCX 章节");
    assert_eq!(book.toc[0].href, "one.xhtml#section");
    let mut utf16 = vec![0xff, 0xfe];
    for unit in "<html><body>中文</body></html>".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    let source = xml(utf16).unwrap();
    assert!(text(parse(&source).unwrap().root()).contains("中文"));
    let mut archive = open(&path).unwrap();
    assert!(read(&mut archive, "one.xhtml", 4).is_err());
}
