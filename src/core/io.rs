//! Loading and saving images, including layered OpenRaster (.ora) documents.

use super::blend::BlendMode;
use super::document::{DocState, Layer};
use super::selection::Selection;
use super::surface::Surface;
use std::io::{Read, Write};
use std::path::Path;

pub const OPEN_EXTS: &[&str] = &["ora", "png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp", "tga", "ico", "qoi"];
pub const SAVE_EXTS: &[&str] = &["ora", "png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp", "tga", "ico", "qoi"];

pub fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn is_layered_format(path: &Path) -> bool {
    ext_of(path) == "ora"
}

pub fn load(path: &Path) -> Result<DocState, String> {
    if ext_of(path) == "ora" {
        return load_ora(path);
    }
    let img = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let s = Surface::from_image(&img);
    Ok(DocState { w: s.w, h: s.h, layers: vec![Layer::from_surface("Background", &s)], active: 0, selection: Selection::none() })
}

pub fn load_surface(path: &Path) -> Result<Surface, String> {
    if ext_of(path) == "ora" {
        return Ok(load_ora(path)?.flatten());
    }
    let img = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    Ok(Surface::from_image(&img))
}

pub fn decode_bytes(bytes: &[u8]) -> Result<Surface, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    Ok(Surface::from_image(&img))
}

pub fn save(path: &Path, st: &DocState, jpeg_quality: u8) -> Result<(), String> {
    let ext = ext_of(path);
    if ext == "ora" {
        return save_ora(path, st);
    }
    let flat = st.flatten();
    let img = flat.to_image();
    match ext.as_str() {
        "jpg" | "jpeg" => {
            let rgb = image::DynamicImage::ImageRgba8(composite_on_white(&flat).to_image()).to_rgb8();
            let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut w = std::io::BufWriter::new(f);
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, jpeg_quality);
            rgb.write_with_encoder(enc).map_err(|e| e.to_string())
        }
        "bmp" | "ico" if false => unreachable!(),
        "ico" => {
            let img = if img.width() > 256 || img.height() > 256 {
                image::imageops::resize(&img, 256.min(img.width()), 256.min(img.height()), image::imageops::FilterType::Lanczos3)
            } else {
                img
            };
            img.save(path).map_err(|e| e.to_string())
        }
        _ => img.save(path).map_err(|e| e.to_string()),
    }
}

fn composite_on_white(s: &Surface) -> Surface {
    let mut out = s.clone();
    for p in out.data.iter_mut() {
        *p = super::surface::over([255, 255, 255, 255], *p, 1.0);
    }
    out
}

pub fn encode_png(s: &Surface) -> Vec<u8> {
    let mut buf = Vec::new();
    s.to_image().write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
    buf
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}
fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&apos;", "'").replace("&amp;", "&")
}

fn save_ora(path: &Path, st: &DocState) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(f);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflate = zip::write::SimpleFileOptions::default();
    let e = |e: zip::result::ZipError| e.to_string();
    z.start_file("mimetype", stored).map_err(e)?;
    z.write_all(b"image/openraster").map_err(|e| e.to_string())?;
    let mut xml = format!("<?xml version='1.0' encoding='UTF-8'?>\n<image version=\"0.0.3\" w=\"{}\" h=\"{}\">\n<stack>\n", st.w, st.h);
    for (i, l) in st.layers.iter().enumerate().rev() {
        let name = format!("data/layer{i}.png");
        xml += &format!(
            "<layer name=\"{}\" src=\"{}\" x=\"0\" y=\"0\" opacity=\"{:.3}\" visibility=\"{}\" composite-op=\"{}\"{}/>\n",
            xml_escape(&l.name),
            name,
            l.opacity,
            if l.visible { "visible" } else { "hidden" },
            l.blend.ora_name(),
            if i == st.active { " selected=\"true\"" } else { "" }
        );
        z.start_file(&name, stored).map_err(e)?;
        z.write_all(&encode_png(&l.px.to_surface())).map_err(|e| e.to_string())?;
    }
    xml += "</stack>\n</image>\n";
    z.start_file("stack.xml", deflate).map_err(e)?;
    z.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
    let flat = st.flatten();
    z.start_file("mergedimage.png", stored).map_err(e)?;
    z.write_all(&encode_png(&flat)).map_err(|e| e.to_string())?;
    let (tw, th) = fit(st.w, st.h, 256);
    let thumb = super::transform::resize_surface(&flat, tw, th, super::transform::Resample::Supersample);
    z.start_file("Thumbnails/thumbnail.png", stored).map_err(e)?;
    z.write_all(&encode_png(&thumb)).map_err(|e| e.to_string())?;
    z.finish().map_err(e)?;
    Ok(())
}

pub fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    let k = (max as f32 / w.max(h) as f32).min(1.0);
    (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1))
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=");
    let i = tag.find(&pat)? + pat.len();
    let q = tag[i..].chars().next()?;
    let rest = &tag[i + 1..];
    let end = rest.find(q)?;
    Some(xml_unescape(&rest[..end]))
}

fn load_ora(path: &Path) -> Result<DocState, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut xml = String::new();
    z.by_name("stack.xml").map_err(|e| e.to_string())?.read_to_string(&mut xml).map_err(|e| e.to_string())?;
    let img_tag_start = xml.find("<image").ok_or("invalid stack.xml")?;
    let img_tag = &xml[img_tag_start..img_tag_start + xml[img_tag_start..].find('>').unwrap_or(0)];
    let w: u32 = attr(img_tag, "w").and_then(|v| v.parse().ok()).ok_or("missing width")?;
    let h: u32 = attr(img_tag, "h").and_then(|v| v.parse().ok()).ok_or("missing height")?;
    let mut layers = Vec::new();
    let mut active = 0;
    let mut pos = 0;
    while let Some(i) = xml[pos..].find("<layer") {
        let start = pos + i;
        let end = start + xml[start..].find('>').ok_or("bad xml")?;
        let tag = &xml[start..end];
        pos = end;
        let src = attr(tag, "src").ok_or("layer without src")?;
        let mut bytes = Vec::new();
        z.by_name(&src).map_err(|e| e.to_string())?.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let s = decode_bytes(&bytes)?;
        let x: i32 = attr(tag, "x").and_then(|v| v.parse().ok()).unwrap_or(0);
        let y: i32 = attr(tag, "y").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut full = Surface::new(w, h);
        for yy in 0..s.h as i32 {
            for xx in 0..s.w as i32 {
                if full.in_bounds(xx + x, yy + y) {
                    full.set(xx + x, yy + y, s.get(xx, yy));
                }
            }
        }
        let mut l = Layer::from_surface(attr(tag, "name").unwrap_or_else(|| "Layer".into()), &full);
        l.opacity = attr(tag, "opacity").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        l.visible = attr(tag, "visibility").map(|v| v != "hidden").unwrap_or(true);
        l.blend = attr(tag, "composite-op").map(|v| BlendMode::from_ora_name(&v)).unwrap_or_default();
        if attr(tag, "selected").as_deref() == Some("true") {
            active = layers.len();
        }
        layers.push(l);
    }
    if layers.is_empty() {
        return Err("no layers in file".into());
    }
    // stack.xml lists top first.
    layers.reverse();
    let active = layers.len() - 1 - active.min(layers.len() - 1);
    Ok(DocState { w, h, layers, active, selection: Selection::none() })
}
