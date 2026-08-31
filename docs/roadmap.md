# Roadmap

| Milestone | Status | Exit criteria |
| --- | --- | --- |
| Survey and decision docs | completed | Vision, roadmap, gap analysis, ADRs exist |
| Cargo skeleton | completed | `cargo build` passes; CLI accepts input PDF |
| Raw image extraction | completed | JPEG (DCTDecode) streams copied losslessly per page |
| Flate image decoding | completed | FlateDecode grayscale/RGB/CMYK images become PNGs |
| Parallel extraction | completed | Pages extract concurrently with rayon; output order stable |
| CBZ output | completed | Default output is a valid CBZ readable in comic readers |
| Fixed-layout EPUB output | completed | `--epub` builds EPUB 3 with one image per page |
| Verification | completed | fmt, clippy, tests, and release build pass |
| Real-input fixes | completed | Indirect DecodeParms predictors resolved; --quality re-encode; EPUB verified visually |
| PDFium render fallback | completed | Layered/tiled PDFs (>512 objects) render full pages; 22 GB bug fixed |
| Optimizations | completed | Per-image grayscale detection; RTL spine; parallel PDFium rendering; batch mode; LTO |
