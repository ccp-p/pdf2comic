use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

mod cbz;
mod epub;
mod extract;

use extract::extract_images;

/// Convert an image-based (manga/comic) PDF to a fixed-layout EPUB.
#[derive(Parser)]
struct Args {
    /// Input PDF file.
    input: PathBuf,
    /// Output file. Defaults to the input name with .epub.
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

    let output = match &args.output {
        Some(p) => p.clone(),
        None => default_output(&args.input, if args.cbz { "cbz" } else { "epub" })?,
    };

    eprintln!("Reading {} ...", args.input.display());
    let quality = if args.lossless { None } else { Some(args.quality) };
    let images = extract_images(&args.input, quality)?;
    eprintln!("Extracted {} images.", images.len());

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
