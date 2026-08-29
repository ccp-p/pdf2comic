## Build Commands

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `cargo build --release`

## Architecture

```text
PDF bytes
   |
   v
extract (lopdf, rayon per page)
  image XObjects -> raw JPEG / decoded PNG
   |
   v
writer
  cbz.rs  -> default ZIP of page images
  epub.rs -> optional fixed-layout EPUB 3
```

## Key Patterns

- `extract` owns PDF parsing and returns ordered page images.
- Writers only consume ordered image data; they never parse PDFs.
- Never re-encode DCTDecode streams.
- Output order must equal PDF page order regardless of worker threads.

## Project Boundary

Personal-use CLI. No DRM handling, no OCR, no GUI. Keep dependencies small
and pure Rust.
