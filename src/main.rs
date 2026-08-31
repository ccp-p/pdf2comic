use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

mod cbz;
mod epub;
mod extract;
mod render;

use extract::extract_images;

/// Convert image-based (manga/comic) PDFs to fixed-layout EPUBs.
#[derive(Parser)]
struct Args {
    /// Input PDF files, or one or more directories of PDFs.
    input: Vec<PathBuf>,
    /// Output file (single input only). Defaults to the input name.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Write a CBZ instead of an EPUB.
    #[arg(long)]
    cbz: bool,
    /// JPEG quality (1-100) used when re-encoding non-JPEG bitmaps.
    #[arg(long, default_value_t = 95)]
    quality: u8,
    /// Keep decoded bitmaps as lossless PNG instead of re-encoding.
    #[arg(long)]
    lossless: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut pdfs: Vec<PathBuf> = Vec::new();
    for input in &args.input {
        if input.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(input)
                .with_context(|| format!("reading {}", input.display()))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false))
                .collect();
            found.sort();
            pdfs.extend(found);
        } else {
            pdfs.push(input.clone());
        }
    }
    if pdfs.is_empty() {
        anyhow::bail!("no PDF files found in the given paths");
    }

    let mut failed = 0usize;
    if args.output.is_some() && pdfs.len() > 1 {
        anyhow::bail!("--output works with a single input file only");
    }
    for pdf in &pdfs {
        let result = run_one(pdf, &args);
        if let Err(e) = result {
            eprintln!("FAILED {}: {:#}", pdf.display(), e);
            failed += 1;
        }
    }
    if failed > 0 {
        anyhow::bail!("{} of {} file(s) failed", failed, pdfs.len());
    }
    Ok(())
}

fn run_one(input: &Path, args: &Args) -> Result<()> {
    let output = match &args.output {
        Some(p) => p.clone(),
        None => default_output(input, if args.cbz { "cbz" } else { "epub" })?,
    };

    let quality = if args.lossless { None } else { Some(args.quality) };
    let images = match extract_images(input, quality) {
        Ok(images) => {
            eprintln!("Extracted {} images.", images.len());
            images
        }
        Err(e) => {
            eprintln!(
                "Direct extraction failed ({}); falling back to PDFium rendering.",
                e
            );
            render::render_pages(input, args.quality)?
        }
    };

    if args.cbz {
        cbz::write_cbz(&output, &images)
    } else {
        epub::write_epub(&output, &images)
    }
    .with_context(|| format!("writing {}", output.display()))?;

    eprintln!("Wrote {} ({} pages).", output.display(), images.len());
    Ok(())
}

fn default_output(input: &Path, ext: &str) -> Result<PathBuf> {
    let stem = input
        .file_stem()
        .with_context(|| format!("input has no file name: {}", input.display()))?;
    let mut out = input.to_path_buf();
    out.set_file_name(stem);
    out.set_extension(ext);
    Ok(out)
}
