use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Convert the EPUB to AZW3 via Calibre, then copy it to a connected
/// Kindle's documents folder when the device mounts as a drive.
pub fn deliver_to_kindle(epub: &Path) -> Result<()> {
    let calibre = find_ebook_convert()?;
    let azw3 = epub.with_extension("azw3");

    eprintln!("Converting to AZW3 with Calibre ...");
    let status = Command::new(&calibre)
        .arg(epub)
        .arg(&azw3)
        .status()
        .with_context(|| format!("running {}", calibre.display()))?;
    if !status.success() {
        bail!("Calibre conversion failed with {}", status);
    }
    eprintln!("Created {}.", azw3.display());

    match find_kindle_documents() {
        Some(docs) => {
            let target = docs.join(
                azw3
                    .file_name()
                    .context("AZW3 path has no file name")?,
            );
            std::fs::copy(&azw3, &target)
                .with_context(|| format!("copying to {}", target.display()))?;
            eprintln!("Copied to {}. Eject the Kindle before unplugging.", target.display());
        }
        None => {
            eprintln!(
                "No Kindle drive detected. Copy {} to the Kindle's \
                 documents folder manually (USB or Calibre device view).",
                azw3.display()
            );
        }
    }
    Ok(())
}

fn find_ebook_convert() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("CALIBRE_EBOOK_CONVERT") {
        return Ok(PathBuf::from(p));
    }
    let candidates = [
        "C:\\Program Files\\Calibre2\\ebook-convert.exe",
        "C:\\Program Files (x86)\\Calibre2\\ebook-convert.exe",
        "D:\\Program Files\\Calibre2\\ebook-convert.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Ok(p);
        }
    }
    bail!(
        "Calibre not found. Install Calibre or set CALIBRE_EBOOK_CONVERT \
         to the full path of ebook-convert.exe"
    )
}

fn find_kindle_documents() -> Option<PathBuf> {
    for letter in 'D'..='Z' {
        let root = PathBuf::from(format!("{letter}:\\"));
        let docs = root.join("documents");
        if docs.is_dir() && root.join("system").is_dir() {
            return Some(docs);
        }
    }
    None
}
