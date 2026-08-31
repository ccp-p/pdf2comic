use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;

pub struct PageImage {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

struct DecodedImage {
    ext: &'static str,
    width: u32,
    height: u32,
    data: Vec<u8>,
}

/// Extract every page image, in PDF page order. Shared images are reused.
pub fn extract_images(path: &Path, quality: Option<u8>) -> Result<Vec<PageImage>> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let doc = Document::load_mem(&bytes).context("parsing PDF")?;

    // Page order per the document's page tree.
    let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    if pages.is_empty() {
        bail!("PDF contains no pages");
    }

    // Per-page image object ids, in XObject name order.
    let page_object_ids: Vec<Vec<ObjectId>> = pages
        .iter()
        .map(|page_id| page_image_ids(&doc, *page_id))
        .collect();

    // Layered/tiled pages (hundreds of image objects per page) are rendered
    // with PDFium; ordinary pages keep the lossless extraction path.
    let tiled: Vec<usize> = page_object_ids
        .iter()
        .enumerate()
        .filter(|(_, ids)| ids.len() > TILED_PAGE_IMAGES)
        .map(|(i, _)| i)
        .collect();
    let rendered: std::collections::HashMap<usize, crate::render::RenderedPage> =
        if tiled.is_empty() {
            Default::default()
        } else {
            eprintln!(
                "{}/{} pages are layered/tiled; rendering those with PDFium ...",
                tiled.len(),
                pages.len()
            );
            crate::render::render_selected(path, quality.unwrap_or(95), &tiled)?
                .into_iter()
                .map(|p| (p.page_index, p))
                .collect()
        };

    // Unique images across ordinary pages, first-use order preserved.
    let mut unique: Vec<ObjectId> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for ids in page_object_ids.iter() {
        if ids.len() > TILED_PAGE_IMAGES {
            continue;
        }
        for id in ids {
            if seen.insert(*id) {
                unique.push(*id);
            }
        }
    }

    let decoded: Vec<Result<DecodedImage>> = unique
        .par_iter()
        .map(|id| decode_image(&doc, *id, quality))
        .collect();

    let mut images: Vec<DecodedImage> = Vec::with_capacity(unique.len());
    for (i, result) in decoded.into_iter().enumerate() {
        images.push(result.with_context(|| format!("decoding image object {}", unique[i].0))?);
    }

    // Flatten to a sequential page-image list.
    let mut index_of = std::collections::HashMap::new();
    for (i, id) in unique.iter().enumerate() {
        index_of.insert(*id, i);
    }

    let mut out = Vec::new();
    let mut counter = 0usize;
    for (page_idx, ids) in page_object_ids.iter().enumerate() {
        if ids.len() > TILED_PAGE_IMAGES {
            counter += 1;
            let r = rendered.get(&page_idx).context("rendered page missing")?;
            out.push(PageImage {
                name: format!("{:04}.jpg", counter),
                width: r.width,
                height: r.height,
                data: r.data.clone(),
            });
            continue;
        }
        for id in ids {
            counter += 1;
            let img = &images[index_of[id]];
            out.push(PageImage {
                name: format!("{:04}.{}", counter, img.ext),
                width: img.width,
                height: img.height,
                data: img.data.clone(),
            });
        }
    }
    Ok(out)
}

const TILED_PAGE_IMAGES: usize = 8;

/// Image XObject ids referenced by one page, sorted by resource name.
fn page_image_ids(doc: &Document, page_id: ObjectId) -> Vec<ObjectId> {
    let mut names: Vec<(String, ObjectId)> = Vec::new();

    // Page -> Resources -> XObject, following indirect references.
    let resources = doc
        .get_dictionary(page_id)
        .ok()
        .and_then(|d| d.get(b"Resources").ok())
        .cloned();
    let Some(resolved) = resources.and_then(|o| resolve(doc, &o).cloned()) else {
        return names.into_iter().map(|(_, id)| id).collect();
    };
    let resources_dict = match &resolved {
        Object::Dictionary(d) => d,
        _ => return Vec::new(),
    };
    let xobjects = resources_dict
        .get(b"XObject")
        .ok()
        .and_then(|o| resolve(doc, o).cloned());
    let Some(xobjects) = xobjects else {
        return Vec::new();
    };

    let dict = match &xobjects {
        Object::Dictionary(d) => d,
        _ => return Vec::new(),
    };
    for (name, value) in dict.iter() {
        let Some(obj) = resolve(doc, value) else {
            continue;
        };
        if is_image_xobject(obj) {
            if let Ok(id) = value.as_reference() {
                names.push((String::from_utf8_lossy(name).to_string(), id));
            }
        }
    }
    names.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    names.into_iter().map(|(_, id)| id).collect()
}

fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok(),
        _ => Some(obj),
    }
}

fn is_image_xobject(obj: &Object) -> bool {
    match obj {
        Object::Stream(s) => s
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| n == b"Image")
            .unwrap_or(false),
        _ => false,
    }
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    // Compare embedded digit runs numerically: Im2 < Im10.
    let numeric = |s: &str| -> (String, u64) {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        (s.to_string(), digits.parse().unwrap_or(0))
    };
    let (an, av) = numeric(a);
    let (bn, bv) = numeric(b);
    av.cmp(&bv).then_with(|| an.cmp(&bn))
}

fn decode_image(doc: &Document, id: ObjectId, quality: Option<u8>) -> Result<DecodedImage> {
    let obj = doc
        .get_object(id)
        .with_context(|| format!("object {} missing", id.0))?;
    let stream = match obj {
        Object::Stream(s) => s,
        _ => bail!("object {} is not a stream", id.0),
    };
    let dict = &stream.dict;

    let width = i64_of(dict.get(b"Width")?)? as u32;
    let height = i64_of(dict.get(b"Height")?)? as u32;
    if width == 0 || height == 0 {
        bail!("image {} has zero dimensions", id.0);
    }

    let filters = filter_names(dict)?;
    if filters
        .iter()
        .any(|f| f == "CCITTFaxDecode" || f == "JBIG2Decode")
    {
        bail!(
            "unsupported image encoding {:?} (rasterization fallback not implemented)",
            filters
        );
    }

    let mut content = stream.content.clone();

    // [Flate, DCT] happens when the JPEG itself is inside a zlib wrapper.
    if filters.first().map(|f| f == "FlateDecode").unwrap_or(false)
        && filters.get(1).map(|f| f == "DCTDecode").unwrap_or(false)
    {
        content = zlib_decompress(&content)?;
    }

    if filters.iter().any(|f| f == "DCTDecode") {
        return Ok(DecodedImage {
            ext: "jpg",
            width,
            height,
            data: content,
        });
    }
    if filters.iter().any(|f| f == "JPXDecode") {
        return Ok(DecodedImage {
            ext: "jp2",
            width,
            height,
            data: content,
        });
    }

    let compressed = filters.iter().any(|f| f == "FlateDecode");
    let raw = if compressed {
        zlib_decompress(&content)?
    } else {
        content
    };

    let colors = colorspace_components(doc, dict)?;
    let bpc = dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| i64_of(o).ok())
        .unwrap_or(8) as u32;
    if bpc != 8 && bpc != 1 {
        bail!("unsupported BitsPerComponent {}", bpc);
    }

    let decode_parms = decode_parms(doc, dict)?;
    let predictor = decode_parms.get("Predictor").copied().unwrap_or(1);
    if predictor >= 2 && bpc != 8 {
        bail!("PNG predictors with {}-bit components are unsupported", bpc);
    }

    let data = if predictor >= 10 {
        unfilter_png(&raw, width, colors)?
    } else {
        raw
    };

    let inverted = decode_inverted(dict);

    // Some PDF producers pad raw bitmaps (e.g. a trailing 2 KiB block).
    if bpc == 8 {
        let expected = width as usize * height as usize * colors;
        if data.len() > expected {
            eprintln!(
                "warning: image {}: {} bytes, expected {}; truncating padding",
                id.0,
                data.len(),
                expected
            );
            return encode_bitmap(width, height, colors, bpc, &data[..expected], inverted, quality);
        }
        if data.len() < expected {
            bail!(
                "image {}: {} bytes, expected {}",
                id.0,
                data.len(),
                expected
            );
        }
    }

    encode_bitmap(width, height, colors, bpc, &data, inverted, quality)
}

fn encode_bitmap(
    width: u32,
    height: u32,
    colors: usize,
    bpc: u32,
    data: &[u8],
    inverted: bool,
    quality: Option<u8>,
) -> Result<DecodedImage> {
    let (color_type, bytes) = match colors {
        1 => expand_1bit_or_gray(data, width, height, bpc),
        3 => (png::ColorType::Rgb, data.to_vec()),
        4 => encode_cmyk_as_rgb(data, width, height, inverted)?,
        other => bail!("unsupported colorspace with {} components", other),
    };
    // Manga scans stored as RGB are usually near-gray; encoding them as
    // grayscale JPEG saves ~50% with no visible loss. Full-color pages
    // (per-image check, conservative threshold) keep their colors.
    let (color_type, bytes) = if color_type == png::ColorType::Rgb {
        match try_to_gray(&bytes) {
            Some(luma) => (png::ColorType::Grayscale, luma),
            None => (png::ColorType::Rgb, bytes),
        }
    } else {
        (color_type, bytes)
    };

    let (ext, data) = match quality {
        None => {
            let mut out = Vec::new();
            let mut encoder = png::Encoder::new(&mut out, width, height);
            encoder.set_color(color_type);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&bytes)?;
            writer.finish()?;
            ("png", out)
        }
        Some(q) => {
            let (color, data) = match color_type {
                png::ColorType::Grayscale => (jpeg_encoder::ColorType::Luma, bytes),
                _ => (jpeg_encoder::ColorType::Rgb, bytes),
            };
            let mut out = Vec::new();
            let encoder = jpeg_encoder::Encoder::new(&mut out, q);
            encoder.encode(&data, width as u16, height as u16, color)?;
            ("jpg", out)
        }
    };

    Ok(DecodedImage {
        ext,
        width,
        height,
        data,
    })
}

/// Returns a grayscale channel when the sampled pixels carry no real
/// chroma; None means "genuinely colorful, keep RGB".
pub(crate) fn try_to_gray(rgb: &[u8]) -> Option<Vec<u8>> {
    for (i, px) in rgb.chunks_exact(3).enumerate() {
        if i % 4 != 0 {
            continue;
        }
        let (r, g, b) = (px[0] as u16, px[1] as u16, px[2] as u16);
        if r.max(g).max(b) - r.min(g).min(b) > 12 {
            return None;
        }
    }
    let mut luma = Vec::with_capacity(rgb.len() / 3);
    for px in rgb.chunks_exact(3) {
        let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
        luma.push(((r * 299 + g * 587 + b * 114) / 1000) as u8);
    }
    Some(luma)
}

fn filter_names(dict: &lopdf::Dictionary) -> Result<Vec<String>> {
    let filter = dict
        .get(b"Filter")
        .map_err(|_| anyhow!("stream has no filter"))?;
    let names = match filter {
        Object::Name(n) => vec![String::from_utf8_lossy(n).to_string()],
        Object::Array(items) => {
            let mut v = Vec::new();
            for item in items {
                match item {
                    Object::Name(n) => v.push(String::from_utf8_lossy(n).to_string()),
                    Object::Reference(id) => {
                        return Err(anyhow!("indirect filter object {}", id.0));
                    }
                    _ => bail!("unexpected filter entry"),
                }
            }
            v
        }
        _ => bail!("unexpected Filter type"),
    };
    Ok(names)
}

fn colorspace_components(doc: &Document, dict: &lopdf::Dictionary) -> Result<usize> {
    let cs = dict
        .get(b"ColorSpace")
        .map_err(|_| anyhow!("image has no ColorSpace"))?;
    let resolved = resolve(doc, cs).with_context(|| "resolving ColorSpace")?;
    match resolved {
        Object::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"G" => Ok(1),
            b"DeviceRGB" | b"RGB" => Ok(3),
            b"DeviceCMYK" | b"CMYK" => Ok(4),
            other => bail!("unsupported colorspace {}", String::from_utf8_lossy(other)),
        },
        Object::Array(items) => {
            let family = items
                .first()
                .and_then(|o| resolve(doc, o))
                .and_then(|o| o.as_name().ok())
                .map(|n| String::from_utf8_lossy(n).to_string())
                .unwrap_or_default();
            match family.as_str() {
                "ICCBased" => {
                    let n = items
                        .get(1)
                        .and_then(|o| resolve(doc, o))
                        .and_then(|o| {
                            if let Object::Stream(s) = o {
                                s.dict.get(b"N").ok().and_then(|x| i64_of(x).ok())
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| anyhow!("ICCBased stream missing N"))?;
                    Ok(n as usize)
                }
                "CalRGB" => Ok(3),
                "CalGray" => Ok(1),
                other => bail!("unsupported colorspace family {}", other),
            }
        }
        other => bail!("unexpected ColorSpace type {:?}", other),
    }
}

fn decode_parms(
    doc: &Document,
    dict: &lopdf::Dictionary,
) -> Result<std::collections::HashMap<String, i64>> {
    let mut out = std::collections::HashMap::new();
    if let Ok(parms) = dict.get(b"DecodeParms") {
        let mut dicts: Vec<&lopdf::Dictionary> = Vec::new();
        match resolve(doc, parms) {
            Some(Object::Dictionary(d)) => dicts.push(d),
            Some(Object::Array(items)) => {
                for item in items {
                    if let Some(Object::Dictionary(d)) = resolve(doc, item) {
                        dicts.push(d);
                    }
                }
            }
            _ => {}
        }
        for d in dicts {
            for key in ["Predictor", "Colors", "BitsPerComponent", "Columns"] {
                if let Ok(v) = d.get(key.as_bytes()) {
                    if let Ok(n) = i64_of(v) {
                        out.insert(key.to_string(), n);
                    }
                }
            }
        }
    }
    Ok(out)
}

fn decode_inverted(dict: &lopdf::Dictionary) -> bool {
    // Adobe CMYK JPEGs are commonly stored inverted: Decode = [1 0 1 0 ...].
    dict.get(b"Decode")
        .ok()
        .and_then(|o| match o {
            Object::Array(items) => items.first().and_then(|x| x.as_float().ok()),
            _ => None,
        })
        .map(|first| first > 0.5)
        .unwrap_or(false)
}

fn i64_of(obj: &Object) -> Result<i64> {
    obj.as_i64()
        .or_else(|_| obj.as_float().map(|f| f as i64))
        .map_err(|_| anyhow!("expected number"))
}

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .read_to_end(&mut out)
        .context("zlib decompression failed")?;
    Ok(out)
}

/// Undo PNG-style row predictors (Predictor 10-15).
fn unfilter_png(data: &[u8], width: u32, colors: usize) -> Result<Vec<u8>> {
    let bpp = colors; // bpc == 8 enforced by caller
    let stride = width as usize * colors;
    let row_len = stride + 1;
    if data.len() < height_rows(data.len(), row_len) * row_len {
        bail!("predictor data shorter than expected");
    }
    let rows = data.len() / row_len;
    let mut out = vec![0u8; rows * stride];
    let mut prev = vec![0u8; stride];
    for r in 0..rows {
        let row_start = r * row_len;
        let filter = data[row_start];
        let row = &mut out[r * stride..(r + 1) * stride];
        row.copy_from_slice(&data[row_start + 1..row_start + 1 + stride]);
        for i in 0..stride {
            let left: i32 = if i >= bpp { row[i - bpp] as i32 } else { 0 };
            let up: i32 = prev[i] as i32;
            let up_left: i32 = if i >= bpp { prev[i - bpp] as i32 } else { 0 };
            let raw: i32 = row[i] as i32;
            row[i] = match filter {
                0 => raw as u8,
                1 => (raw + left) as u8,
                2 => (raw + up) as u8,
                3 => (raw + (left + up) / 2) as u8,
                4 => {
                    let p = left + up - up_left;
                    let pa = (p - left).abs();
                    let pb = (p - up).abs();
                    let pc = (p - up_left).abs();
                    let pred = if pa <= pb && pa <= pc {
                        left
                    } else if pb <= pc {
                        up
                    } else {
                        up_left
                    };
                    (raw + pred) as u8
                }
                other => bail!("unknown PNG filter type {}", other),
            };
        }
        prev.copy_from_slice(row);
    }
    Ok(out)
}

fn height_rows(data_len: usize, row_len: usize) -> usize {
    data_len / row_len.max(1)
}

fn expand_1bit_or_gray(
    data: &[u8],
    width: u32,
    height: u32,
    bpc: u32,
) -> (png::ColorType, Vec<u8>) {
    if bpc == 1 {
        let w = width as usize;
        let mut out = Vec::with_capacity(w * height as usize);
        let stride = w.div_ceil(8);
        for y in 0..height as usize {
            let row = &data[y * stride..(y + 1) * stride];
            for x in 0..w {
                let byte = row[x / 8];
                let bit = (byte >> (7 - (x % 8))) & 1;
                let v = if bit == 1 { 255 } else { 0 };
                out.push(v);
            }
        }
        (png::ColorType::Grayscale, out)
    } else {
        (png::ColorType::Grayscale, data.to_vec())
    }
}

fn encode_cmyk_as_rgb(
    data: &[u8],
    width: u32,
    height: u32,
    inverted: bool,
) -> Result<(png::ColorType, Vec<u8>)> {
    let pixels = (width as usize) * (height as usize);
    if data.len() < pixels * 4 {
        bail!("CMYK data shorter than W*H*4");
    }
    let mut out = Vec::with_capacity(pixels * 3);
    for px in 0..pixels {
        let mut c = data[px * 4] as u32;
        let mut m = data[px * 4 + 1] as u32;
        let mut y = data[px * 4 + 2] as u32;
        let mut k = data[px * 4 + 3] as u32;
        if inverted {
            c = 255 - c;
            m = 255 - m;
            y = 255 - y;
            k = 255 - k;
        }
        let r = 255 - u32::min(255, c + k);
        let g = 255 - u32::min(255, m + k);
        let b = 255 - u32::min(255, y + k);
        out.extend_from_slice(&[r as u8, g as u8, b as u8]);
    }
    Ok((png::ColorType::Rgb, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order_sorts_digit_runs() {
        let mut names = vec!["Im10", "Im2", "Im1"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, vec!["Im1", "Im2", "Im10"]);
    }

    #[test]
    fn unfilter_handles_sub_filter() {
        // Two rows of 2 gray pixels, Sub filter on second pixel of row 1.
        let data = vec![0u8, 10, 20, 1, 5, 0];
        let out = unfilter_png(&data, 2, 1).unwrap();
        assert_eq!(out, vec![10, 20, 5, 5]);
    }

    #[test]
    fn cmyk_conversion_is_invertible_contract() {
        let data = vec![0, 0, 0, 255];
        let (_, rgb) = encode_cmyk_as_rgb(&data, 1, 1, false).unwrap();
        assert_eq!(rgb, vec![0, 0, 0]);
    }

    #[test]
    fn near_gray_image_converts_to_luma() {
        let rgb = vec![100u8, 102, 98, 200, 201, 199];
        let luma = try_to_gray(&rgb).unwrap();
        assert_eq!(luma.len(), 2);
        assert!(luma[0] > 90 && luma[0] < 110);
    }

    #[test]
    fn colorful_image_stays_rgb() {
        let rgb = vec![200u8, 30, 30, 30, 200, 30];
        assert!(try_to_gray(&rgb).is_none());
    }
}
