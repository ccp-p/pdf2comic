# ADR-0002: CBZ default, fixed-layout EPUB optional

## Status

Accepted; default flipped to EPUB per user preference (see ADR-0003)

## Context

For image-only books there are two realistic formats. CBZ is a plain ZIP of
page images understood by every comic reader (Mihon, YACReader, Panels, most
e-ink readers). EPUB fixed-layout is more universal as an "ebook" but wraps
the same images in XHTML and metadata.

## Decision

1. Default output is CBZ: simplest structure, no overhead, trivially correct.
2. `--epub` produces an EPUB 3 fixed-layout book: one XHTML page per image,
   full-bleed, with nav and spine.
3. Both formats share the same extracted image pipeline; the writer is a
   thin layer on top.

## Consequences

- One extraction path to test; two trivial writers.
- EPUB output stays image-identical to CBZ output.
