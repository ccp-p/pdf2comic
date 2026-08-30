# ADR-0005: Calibre AZW3 delivery for large books

## Status

Accepted

## Context

The user reads large manga EPUBs (100-300 MB). Amazon Send to Kindle
caps web uploads at 200 MB and e-ink Kindles do not open EPUB files
copied over USB, so a local conversion step is required.

## Decision

1. `--kindle` converts the produced EPUB to AZW3 with Calibre's
   `ebook-convert` CLI (no size limit, offline).
2. If a Kindle is mounted as a USB drive (documents + system folders),
   the AZW3 is copied straight into its documents folder.
3. If no device is found, the AZW3 stays next to the EPUB for manual copy.
4. Calibre location comes from `CALIBRE_EBOOK_CONVERT` or standard
   install paths.

## Consequences

* One command turns a PDF into a file the Kindle can actually open.
* Calibre becomes a runtime dependency only when `--kindle` is used.
* MTP-only Kindles (no drive letter) still need a manual copy step.
