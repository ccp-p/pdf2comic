use std::path::Path;

use anyhow::{Context, Result};
use pdfium_render::prelude::*;

use crate::extract::PageImage;

pub struct RenderedPage {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Fallback path for PDFs whose pages are compositions of many image
/// objects (layered or tiled scans). Renders each page to a single JPEG
/// with PDFium at 2x scale (about 144 dpi).
pub fn render_pages(pdf_path: &Path, quality: u8) -> Result<Vec<PageImage>> {
    let rendered = render_selected(pdf_path, quality, &[])?;
    Ok(rendered
        .into_iter()
        .enumerate()
        .map(|(i, p)| PageImage {
            name: format!("{:04}.jpg", i + 1),
            width: p.width,
            height: p.height,
            data: p.data,
        })
        .collect())
}

/// Render the given 0-based page indices (empty = all pages). Workers run
/// in parallel; each thread binds its own PDFium instance and document.
pub fn render_selected(
    pdf_path: &Path,
    quality: u8,
    page_indices: &[usize],
) -> Result<Vec<RenderedPage>> {
    let total = {
        let pdfium = bind_pdfium()?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .with_context(|| format!("opening {} with PDFium", pdf_path.display()))?;
        document.pages().len() as usize
    };
    let indices: Vec<usize> = if page_indices.is_empty() {
        (0..total).collect()
    } else {
        page_indices.to_vec()
    };
    eprintln!("Rendering {} pages with PDFium ...", indices.len());

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(indices.len().max(1));
    let chunk_size = indices.len().div_ceil(workers);
    let chunks: Vec<&[usize]> = indices.chunks(chunk_size).collect();

    let results: Vec<Result<Vec<RenderedPage>>> = std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                s.spawn(move || render_chunk(pdf_path, quality, chunk))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut out = Vec::with_capacity(indices.len());
    for result in results {
        out.extend(result?);
    }
    Ok(out)
}

fn render_chunk(
    pdf_path: &Path,
    quality: u8,
    indices: &[usize],
) -> Result<Vec<RenderedPage>> {
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .with_context(|| format!("opening {} with PDFium", pdf_path.display()))?;

    let mut out = Vec::with_capacity(indices.len());
    for &index in indices {
        let page = document
            .pages()
            .get(index as u16)
            .with_context(|| format!("getting page {}", index + 1))?;
        let config = PdfRenderConfig::new().scale_page_by_factor(2.0);
        let bitmap = page
            .render_with_config(&config)
            .with_context(|| format!("rendering page {}", index + 1))?;
        let (w, h) = (bitmap.width(), bitmap.height());
        let rgba = bitmap.as_rgba_bytes();

        let (w, h) = (w.max(0) as u32, h.max(0) as u32);
        let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
        for px in rgba.chunks_exact(4) {
            rgb.push(px[0]);
            rgb.push(px[1]);
            rgb.push(px[2]);
        }

        let (color, rgb) = match crate::extract::try_to_gray(&rgb) {
            Some(luma) => (jpeg_encoder::ColorType::Luma, luma),
            None => (jpeg_encoder::ColorType::Rgb, rgb),
        };
        let mut data = Vec::new();
        let encoder = jpeg_encoder::Encoder::new(&mut data, quality);
        encoder.encode(
            &rgb,
            w.min(u16::MAX as u32) as u16,
            h.min(u16::MAX as u32) as u16,
            color,
        )?;

        out.push(RenderedPage {
            page_index: index,
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
