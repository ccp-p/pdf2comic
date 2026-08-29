use std::io::Write;
use std::path::Path;

use anyhow::Result;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::extract::PageImage;

pub fn write_epub(path: &Path, images: &[PageImage]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // mimetype must be the first, uncompressed entry.
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/epub+zip")?;

    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="utf-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#,
    )?;

    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "comic".into());
    let uid = format!("urn:uuid:{}", uuid_v4());

    // Manifest + spine.
    let mut manifest = String::new();
    let mut spine = String::new();
    for (i, img) in images.iter().enumerate() {
        manifest.push_str(&format!(
            "    <item id=\"p{}\" href=\"pages/page{:04}.xhtml\" media-type=\"application/xhtml+xml\"/>\n",
            i + 1,
            i + 1
        ));
        manifest.push_str(&format!(
            "    <item id=\"img{}\" href=\"images/{}\" media-type=\"{}\"/>\n",
            i + 1,
            img.name,
            media_type(&img.name)
        ));
        spine.push_str(&format!("    <itemref idref=\"p{}\"/>\n", i + 1));
    }

    let opf = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="uid" xml:lang="ja">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">{uid}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:language>ja</dc:language>
    <meta property="dcterms:modified">{modified}Z</meta>
    <meta property="rendition:layout">pre-paginated</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="style.css" media-type="text/css"/>
{manifest}  </manifest>
  <spine>
{spine}  </spine>
</package>
"#,
        uid = uid,
        title = xml_escape(&title),
        modified = modified_utc(),
        manifest = manifest,
        spine = spine,
    );
    zip.start_file("OEBPS/content.opf", deflated)?;
    zip.write_all(opf.as_bytes())?;

    // EPUB 3 navigation document.
    let mut nav_items = String::new();
    for (i, _) in images.iter().enumerate() {
        nav_items.push_str(&format!(
            "      <li><a href=\"pages/page{:04}.xhtml\">Page {}</a></li>\n",
            i + 1,
            i + 1
        ));
    }
    let nav = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>{title}</title></head>
<body>
  <nav epub:type="toc">
    <ol>
{nav_items}    </ol>
  </nav>
</body>
</html>
"#,
        title = xml_escape(&title),
        nav_items = nav_items,
    );
    zip.start_file("OEBPS/nav.xhtml", deflated)?;
    zip.write_all(nav.as_bytes())?;

    zip.start_file("OEBPS/style.css", deflated)?;
    zip.write_all(
        b"body{margin:0;padding:0;background:#000;} svg{display:block;width:100%;height:100%;}\n",
    )?;

    for (i, img) in images.iter().enumerate() {
        let page = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head>
  <title>Page {n}</title>
  <link rel="stylesheet" type="text/css" href="../style.css"/>
</head>
<body>
  <svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {w} {h}">
    <image width="{w}" height="{h}" xlink:href="../images/{img}"/>
  </svg>
</body>
</html>
"#,
            n = i + 1,
            w = img.width,
            h = img.height,
            img = img.name,
        );
        zip.start_file(format!("OEBPS/pages/page{:04}.xhtml", i + 1), deflated)?;
        zip.write_all(page.as_bytes())?;

        zip.start_file(format!("OEBPS/images/{}", img.name), stored)?;
        zip.write_all(&img.data)?;
    }

    zip.finish()?;
    Ok(())
}

fn media_type(name: &str) -> &'static str {
    if name.ends_with(".jpg") {
        "image/jpeg"
    } else if name.ends_with(".png") {
        "image/png"
    } else {
        "image/jp2"
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn modified_utc() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map(|secs| {
            // YYYY-MM-DDTHH:MM:SS, good enough for dcterms:modified.
            let days = secs / 86400;
            let rem = secs % 86400;
            let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
            let (y, mo, d) = civil_from_days(days as i64);
            format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y, mo, d, h, m, s)
        })
        .unwrap_or_else(|_| "1970-01-01T00:00:00".into())
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    // Howard Hinnant's civil_from_days algorithm.
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn uuid_v4() -> String {
    // Random enough for a personal tool; avoids another dependency.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut state = nanos ^ (std::process::id() as u128) << 64;
    let mut bytes = [0u8; 16];
    for b in bytes.iter_mut() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (state >> 64) as u8;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
