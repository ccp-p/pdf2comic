# Gap Analysis

| Common PDF-to-EPUB approach | Decision for comic PDFs |
| --- | --- |
| Extract text and rebuild reflowable HTML | Image PDFs have no usable text; skip entirely |
| Rasterize pages with a rendering engine | Copy embedded image streams directly when possible |
| Re-encode images for smaller files | Keep original bytes; no quality loss, no CPU cost |
| EPUB as the only output | CBZ is the natural comic format; EPUB fixed-layout is optional |
| Sequential per-page processing | Parallel page extraction with deterministic output order |
| Reading order heuristics | Trust PDF page order; manga panel order is baked into images |

Behavior intentionally preserved: page resolution, image quality, and page
count identical to the source PDF.
