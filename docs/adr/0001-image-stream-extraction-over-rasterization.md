# ADR-0001: Image stream extraction over rasterization

## Status

Accepted

## Context

Manga PDFs contain full-page images. A general PDF converter would render
pages with an engine like PDFium or MuPDF and screenshot them. That needs a
heavy native dependency, costs CPU, and usually re-encodes (and degrades)
images.

## Decision

1. Parse the PDF with `lopdf` (pure Rust) and read image XObjects per page.
2. Copy `DCTDecode` (JPEG) streams byte-for-byte to the output.
3. Decode `FlateDecode` bitmaps and re-encode as PNG only when necessary.
4. Fall back to rasterization only if a page uses encodings we cannot handle,
   and report it loudly instead of silently producing blank pages.

## Consequences

- No native rendering library; the binary stays pure Rust and portable.
- JPEG pages convert at IO speed with zero quality loss.
- Exotic encodings (JBIG2, CCITT) need explicit fallback work later.
