# Vision

## Goal

Convert image-based (manga/comic) PDF files into reader-friendly ebook
formats without re-encoding page images. One small Rust binary should
produce:

- Fixed-layout EPUB by default: one full-page image per XHTML page.
- CBZ on request (`--cbz`): a ZIP of sequentially named page images.

JPEG streams inside the PDF must be copied byte-for-byte (lossless, fast).
Other image encodings are converted with minimal processing.

## Non-goals

- Text extraction, OCR, or reflowable text conversion.
- Re-compressing or downscaling page images.
- Reading-order detection beyond PDF page order.
- GUI; this is a command-line tool.
- DRM-protected PDF support.

## Product Shape

```
pdf2comic input.pdf              -> input.epub
pdf2comic input.pdf --cbz       -> input.cbz
```
