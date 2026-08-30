# ADR-0004: PDFium render fallback for layered/tiled pages

## Status

Accepted

## Context

Some manga PDFs stack hundreds of nearly full-page JPEGs per page
(layered scans). Direct extraction multiplies overlapping content: one
real PDF produced 42k image objects and a 22 GB EPUB.

## Decision

1. If a PDF yields more than 512 image objects, or extraction fails on
   unsupported encodings, render every page with PDFium at 2x scale
   (~144 dpi) and encode JPEG at the configured quality.
2. PDFium is loaded dynamically: `pdfium.dll` next to the executable, the
   `PDFIUM_DLL` env var, or the system library path.
3. Direct extraction remains the primary path for normal scan PDFs.

## Consequences

* Layered PDFs convert correctly and at a sane size (22 GB -> 175 MB).
* Rendering is CPU-bound and sequential; acceptable for personal use.
* A pdfium.dll runtime dependency exists only for the fallback path.
