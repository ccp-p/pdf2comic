use std::path::Path;

use anyhow::{Context, Result};
use pdfium_render::prelude::*;

use crate::extract::PageImage;

/// Fallback path for PDFs whose pages are compositions of many image
/// objects (layered or tiled scans). Renders each page to a single JPEG
/// with PDFium at 2x scale (about 144 dpi).
pub fn render_pages(pdf_path: &Path, quality: u8) -> Result<Vec<PageImage>> {
    let pdfium = bind_pdfium()?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .with_context(|| format!("opening {} with PDFium", pdf_path.display()))?;

    let page_count = document.pages().len() as usize;
    eprintln!("Rendering {} pages with PDFium ...", page_count);

    // PDFium objects are not safe to share across threads; render sequentially.
    let mut out = Vec::with_capacity(page_count);
    for (i, page) in document.pages().iter().enumerate() {
        let config = PdfRenderConfig::new().scale_page_by_factor(2.0);
        let bitmap = page
            .render_with_config(&config)
            .with_context(|| format!("rendering page {}", i + 1))?;
        let (w, h) = (bitmap.width(), bitmap.height());
        let rgba = bitmap.as_rgba_bytes();

        let (w, h) = (w.max(0) as u32, h.max(0) as u32);
        let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
        for px in rgba.chunks_exact(4) {
            rgb.push(px[0]);
            rgb.push(px[1]);
            rgb.push(px[2]);
        }

        let mut data = Vec::new();
        let encoder = jpeg_encoder::Encoder::new(&mut data, quality);
        encoder.encode(
            &rgb,
            w.min(u16::MAX as u32) as u16,
            h.min(u16::MAX as u32) as u16,
            jpeg_encoder::ColorType::Rgb,
        )?;

        out.push(PageImage {
            name: format!("{:04}.jpg", i + 1),
            width: w,
            height: h,
            data,
        });
    }

    Ok(out)
}

fn bind_pdfium() -> Result<Pdfium> {
    let candidates = [
        std::env::var("PDFIUM_DLL").ok(),
        Some("pdfium.dll".to_string()),
        Some("D:\\software\\commonTool\\foxmail\\pdfium.dll".to_string()),
    ];
    for candidate in candidates.into_iter().flatten() {
        let path = Path::new(&candidate);
        if path.exists() {
            if let Ok(bindings) = Pdfium::bind_to_library(path) {
                return Ok(Pdfium::new(bindings));
            }
        }
    }
    // Last resort: search the system library path.
    let bindings = Pdfium::bind_to_system_library().context(
        "PDFium not found. Set PDFIUM_DLL to a pdfium.dll path \
         or place pdfium.dll next to pdf2comic.exe",
    )?;
    Ok(Pdfium::new(bindings))
}
