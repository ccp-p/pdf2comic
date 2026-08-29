# ADR-0003: Quality-controlled JPEG re-encode for non-JPEG bitmaps

## Status

Accepted (user-driven revision of ADR-0001)

## Context

The first real-world manga PDF stores most pages as FlateDecode 24-bit
bitmaps with PNG predictors. Keeping them lossless as PNG inflates a
480 MB PDF to about 1 GB, and the user wants EPUB output at a sane size.

## Decision

1. DCTDecode (JPEG) streams are still copied byte-for-byte.
2. Decoded bitmaps are re-encoded as JPEG at `--quality` (default 95;
   the encoder already uses 4:4:4 chroma sampling at quality >= 90),
   which matches how the same scans are stored elsewhere.
3. `--lossless` keeps PNG output when fidelity matters more than size.
4. Predictor handling must resolve indirect `/DecodeParms` references;
   ignoring them produces salt-and-pepper noise (found on real input).

## Consequences

* Output size stays close to the source PDF for typical scans.
* Bitmap pages gain a small, user-controlled quality loss.
* Lossless remains available behind an explicit flag.
