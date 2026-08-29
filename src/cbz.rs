use std::io::Write;
use std::path::Path;

use anyhow::Result;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::extract::PageImage;

pub fn write_cbz(path: &Path, images: &[PageImage]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for img in images {
        zip.start_file(&img.name, options)?;
        zip.write_all(&img.data)?;
    }
    zip.finish()?;
    Ok(())
}
