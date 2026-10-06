//! Text rasterization using system fonts.

use super::geom::Rect;
use super::paint::Cov;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::collections::HashMap;
use std::sync::Arc;

pub struct FontLib {
    db: fontdb::Database,
    pub families: Vec<String>,
    cache: HashMap<fontdb::ID, Arc<Vec<u8>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// 0 left, 1 center, 2 right
    pub align: i32,
    pub aa: bool,
}

pub struct TextLayout {
    pub cov: Cov,
    /// Caret rect (document coordinates) at the end of the text.
    pub caret: Rect,
    pub bounds: Rect,
}

const BUNDLED: &[u8] = include_bytes!("../../ui/fonts/Inter-400.ttf");

impl FontLib {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db.load_font_data(BUNDLED.to_vec());
        let mut families: Vec<String> = db
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .filter(|n| !n.starts_with('.'))
            .collect();
        families.sort_by_key(|a| a.to_lowercase());
        families.dedup();
        FontLib { db, families, cache: HashMap::new() }
    }

    fn face(&mut self, family: &str, bold: bool, italic: bool) -> Option<(Arc<Vec<u8>>, u32, bool, bool)> {
        let fam = [fontdb::Family::Name(family), fontdb::Family::SansSerif];
        let q = fontdb::Query {
            families: &fam,
            weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
            stretch: fontdb::Stretch::Normal,
            style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
        };
        let id = self.db.query(&q).or_else(|| self.db.faces().next().map(|f| f.id))?;
        let info = self.db.face(id)?;
        let synth_bold = bold && info.weight.0 < 600;
        let synth_italic = italic && info.style == fontdb::Style::Normal;
        let index = info.index;
        if !self.cache.contains_key(&id) {
            let data = self.db.with_face_data(id, |d, _| d.to_vec())?;
            self.cache.insert(id, Arc::new(data));
        }
        Some((self.cache[&id].clone(), index, synth_bold, synth_italic))
    }

    /// Lays out and rasterizes `text` with its top-left at (x, y).
    pub fn render(&mut self, text: &str, style: &TextStyle, x: f32, y: f32, doc: Rect) -> TextLayout {
        let mut cov = Cov::new(doc);
        let Some((data, index, sb, si)) = self.face(&style.family, style.bold, style.italic) else {
            return TextLayout { cov, caret: Rect::new(x as i32, y as i32, x as i32 + 1, (y + style.size) as i32), bounds: Rect::EMPTY };
        };
        let Ok(font) = FontRef::try_from_slice_and_index(&data, index) else {
            return TextLayout { cov, caret: Rect::new(x as i32, y as i32, x as i32 + 1, (y + style.size) as i32), bounds: Rect::EMPTY };
        };
        let scale = PxScale::from(style.size * 1.333);
        let sf = font.as_scaled(scale);
        let line_h = (sf.ascent() - sf.descent() + sf.line_gap()).ceil();
        let lines: Vec<&str> = text.split('\n').collect();
        let widths: Vec<f32> = lines
            .iter()
            .map(|l| {
                let mut w = 0.0;
                let mut prev = None;
                for ch in l.chars() {
                    let g = sf.glyph_id(ch);
                    if let Some(p) = prev {
                        w += sf.kern(p, g);
                    }
                    w += sf.h_advance(g);
                    prev = Some(g);
                }
                w
            })
            .collect();
        let max_w = widths.iter().cloned().fold(0.0, f32::max);
        let mut bounds = Rect::EMPTY;
        let mut caret = Rect::EMPTY;
        for (li, line) in lines.iter().enumerate() {
            let lx = match style.align {
                1 => x + (max_w - widths[li]) / 2.0,
                2 => x + (max_w - widths[li]),
                _ => x,
            };
            let baseline = y + li as f32 * line_h + sf.ascent();
            let mut pen = lx;
            let mut prev = None;
            for ch in line.chars() {
                let gid = sf.glyph_id(ch);
                if let Some(p) = prev {
                    pen += sf.kern(p, gid);
                }
                let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen, baseline));
                if let Some(outline) = font.outline_glyph(glyph) {
                    let b = outline.px_bounds();
                    outline.draw(|gx, gy, c| {
                        let py = b.min.y as i32 + gy as i32;
                        let shear = if si { ((baseline - py as f32) * 0.2).round() as i32 } else { 0 };
                        let px = b.min.x as i32 + gx as i32 + shear;
                        let v = if style.aa { c } else if c >= 0.5 { 1.0 } else { 0.0 };
                        let v = (v * 255.0) as u8;
                        cov.max(px, py, v);
                        if sb {
                            let extra = (style.size / 24.0).ceil().max(1.0) as i32;
                            for e in 1..=extra {
                                cov.max(px + e, py, v);
                            }
                        }
                    });
                    bounds = bounds.union(&Rect::new(b.min.x as i32 - 2, b.min.y as i32, b.max.x as i32 + 4, b.max.y as i32 + 1));
                }
                pen += sf.h_advance(gid);
                prev = Some(gid);
            }
            if style.underline && !line.is_empty() {
                let uy = baseline + (style.size * 0.12).max(1.0);
                let th = (style.size / 14.0).max(1.0);
                for yy in uy as i32..(uy + th) as i32 {
                    for xx in lx as i32..(lx + widths[li]) as i32 {
                        cov.max(xx, yy, 255);
                    }
                }
            }
            let top = (baseline - sf.ascent()) as i32;
            bounds = bounds.union(&Rect::new(lx as i32, top, (lx + widths[li]) as i32 + 1, top + line_h as i32));
            if li == lines.len() - 1 {
                caret = Rect::new(pen as i32, top, pen as i32 + 1.max((style.size / 20.0) as i32), top + line_h as i32);
            }
        }
        cov.touch(bounds.inflate(4));
        TextLayout { cov, caret, bounds }
    }
}
