//! Pixel selections represented as coverage masks.

use super::geom::{Pt, Rect};
use rayon::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Replace,
    Union,
    Exclude,
    Intersect,
    Xor,
}

impl SelectionMode {
    pub fn from_index(i: i32) -> Self {
        match i {
            1 => SelectionMode::Union,
            2 => SelectionMode::Exclude,
            3 => SelectionMode::Intersect,
            4 => SelectionMode::Xor,
            _ => SelectionMode::Replace,
        }
    }
}

/// An axis aligned outline segment in document coordinates (pixel edges).
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

#[derive(Clone, Debug)]
pub struct Mask {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
    pub bounds: Rect,
    pub outline: Vec<Seg>,
}

impl Mask {
    pub fn empty(w: u32, h: u32) -> Mask {
        Mask { w, h, data: vec![0; (w * h) as usize], bounds: Rect::EMPTY, outline: vec![] }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            0
        } else {
            self.data[(y as u32 * self.w + x as u32) as usize]
        }
    }

    /// Recomputes bounds and outline. Must be called after editing data.
    pub fn finish(mut self) -> Mask {
        let (w, h) = (self.w as i32, self.h as i32);
        let mut b = Rect::EMPTY;
        for y in 0..h {
            let row = &self.data[(y * w) as usize..((y + 1) * w) as usize];
            if let Some(first) = row.iter().position(|v| *v >= 128) {
                let last = row.iter().rposition(|v| *v >= 128).unwrap();
                b = b.union(&Rect::new(first as i32, y, last as i32 + 1, y + 1));
            }
        }
        self.bounds = b;
        let inside = |x: i32, y: i32| self.get(x, y) >= 128;
        let mut segs = Vec::new();
        if !b.is_empty() {
            // Horizontal edges.
            for y in b.y0..=b.y1 {
                let mut run: Option<i32> = None;
                for x in b.x0..=b.x1 {
                    let edge = x < b.x1 && inside(x, y) != inside(x, y - 1);
                    match (edge, run) {
                        (true, None) => run = Some(x),
                        (false, Some(s)) => {
                            segs.push(Seg { x0: s, y0: y, x1: x, y1: y });
                            run = None;
                        }
                        _ => {}
                    }
                }
            }
            // Vertical edges.
            for x in b.x0..=b.x1 {
                let mut run: Option<i32> = None;
                for y in b.y0..=b.y1 {
                    let edge = y < b.y1 && inside(x, y) != inside(x - 1, y);
                    match (edge, run) {
                        (true, None) => run = Some(y),
                        (false, Some(s)) => {
                            segs.push(Seg { x0: x, y0: s, x1: x, y1: y });
                            run = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        self.outline = segs;
        self
    }

    pub fn rect(w: u32, h: u32, r: Rect) -> Mask {
        let mut m = Mask::empty(w, h);
        let r = r.intersect(&Rect::from_size(w, h));
        for y in r.y0..r.y1 {
            m.data[(y as u32 * w + r.x0 as u32) as usize..(y as u32 * w + r.x1 as u32) as usize].fill(255);
        }
        m.finish()
    }

    pub fn ellipse(w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Mask {
        let mut m = Mask::empty(w, h);
        let cx = (x0 + x1) / 2.0;
        let cy = (y0 + y1) / 2.0;
        let rx = ((x1 - x0) / 2.0).abs().max(0.01);
        let ry = ((y1 - y0) / 2.0).abs().max(0.01);
        let r = Rect::from_points(x0, y0, x1, y1).inflate(1).intersect(&Rect::from_size(w, h));
        let ww = w as usize;
        m.data.par_chunks_mut(ww).enumerate().for_each(|(y, row)| {
            let y = y as i32;
            if y < r.y0 || y >= r.y1 {
                return;
            }
            for x in r.x0..r.x1 {
                let mut cov = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                        let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                        let dx = (px - cx) / rx;
                        let dy = (py - cy) / ry;
                        if dx * dx + dy * dy <= 1.0 {
                            cov += 1;
                        }
                    }
                }
                row[x as usize] = (cov * 255 / 16) as u8;
            }
        });
        m.finish()
    }

    /// Polygon (lasso) mask, even-odd rule with 4x vertical supersampling.
    pub fn polygon(w: u32, h: u32, pts: &[Pt]) -> Mask {
        let mut m = Mask::empty(w, h);
        if pts.len() < 3 {
            return m.finish();
        }
        let ww = w as usize;
        m.data.par_chunks_mut(ww).enumerate().for_each(|(y, row)| {
            let mut acc = vec![0u16; ww];
            for s in 0..4 {
                let sy = y as f32 + (s as f32 + 0.5) / 4.0;
                let mut xs: Vec<f32> = Vec::new();
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    if (a.y <= sy && b.y > sy) || (b.y <= sy && a.y > sy) {
                        let t = (sy - a.y) / (b.y - a.y);
                        xs.push(a.x + t * (b.x - a.x));
                    }
                }
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for pair in xs.chunks(2) {
                    if pair.len() < 2 {
                        break;
                    }
                    let xa = pair[0].max(0.0);
                    let xb = pair[1].min(w as f32);
                    if xb <= xa {
                        continue;
                    }
                    let ia = xa.floor() as usize;
                    let ib = (xb.ceil() as usize).min(ww);
                    for x in ia..ib {
                        let l = (x as f32).max(xa);
                        let r = (x as f32 + 1.0).min(xb);
                        if r > l {
                            acc[x] += ((r - l) * 64.0) as u16;
                        }
                    }
                }
            }
            for x in 0..ww {
                row[x] = (acc[x] as u32 * 255 / 256).min(255) as u8;
            }
        });
        m.finish()
    }

    pub fn from_coverage(w: u32, h: u32, data: Vec<u8>) -> Mask {
        Mask { w, h, data, bounds: Rect::EMPTY, outline: vec![] }.finish()
    }

    pub fn combine(&self, other: &Mask, mode: SelectionMode) -> Mask {
        let data = self
            .data
            .par_iter()
            .zip(other.data.par_iter())
            .map(|(&a, &b)| match mode {
                SelectionMode::Replace => b,
                SelectionMode::Union => a.max(b),
                SelectionMode::Exclude => ((a as u16 * (255 - b) as u16) / 255) as u8,
                SelectionMode::Intersect => ((a as u16 * b as u16) / 255) as u8,
                SelectionMode::Xor => {
                    let (a, b) = (a as i32, b as i32);
                    (a + b - 2 * a * b / 255).clamp(0, 255) as u8
                }
            })
            .collect();
        Mask::from_coverage(self.w, self.h, data)
    }

    pub fn invert(&self) -> Mask {
        Mask::from_coverage(self.w, self.h, self.data.iter().map(|v| 255 - v).collect())
    }

    /// Returns true when every non-zero pixel is fully selected and the shape is the bounding rect.
    pub fn is_rectangular(&self) -> bool {
        let b = self.bounds;
        if b.is_empty() {
            return false;
        }
        for y in b.y0..b.y1 {
            for x in b.x0..b.x1 {
                if self.get(x, y) != 255 {
                    return false;
                }
            }
        }
        self.data.iter().filter(|v| **v != 0).count() == (b.width() * b.height()) as usize
    }
}

/// The active selection of a document. `None` means "nothing selected" (everything editable).
#[derive(Clone, Default)]
pub struct Selection {
    pub mask: Option<Arc<Mask>>,
}

impl Selection {
    pub fn none() -> Self {
        Selection { mask: None }
    }
    pub fn from_mask(m: Mask) -> Self {
        if m.bounds.is_empty() {
            Selection { mask: None }
        } else {
            Selection { mask: Some(Arc::new(m)) }
        }
    }
    pub fn is_active(&self) -> bool {
        self.mask.is_some()
    }
    /// Coverage 0..255 for painting. 255 everywhere when nothing is selected.
    #[inline]
    pub fn coverage(&self, x: i32, y: i32) -> u8 {
        match &self.mask {
            None => 255,
            Some(m) => m.get(x, y),
        }
    }
    /// Editable bounds within a document of the given size.
    pub fn bounds(&self, w: u32, h: u32) -> Rect {
        match &self.mask {
            None => Rect::from_size(w, h),
            Some(m) => m.bounds,
        }
    }
    /// Combines a freshly made shape mask with the current selection.
    pub fn apply(&self, shape: Mask, mode: SelectionMode) -> Selection {
        match (&self.mask, mode) {
            (_, SelectionMode::Replace) | (None, _) => {
                if mode == SelectionMode::Exclude || mode == SelectionMode::Intersect {
                    if self.mask.is_none() {
                        // Excluding from "nothing" leaves nothing; intersect too.
                        return Selection::none();
                    }
                }
                Selection::from_mask(shape)
            }
            (Some(cur), m) => Selection::from_mask(cur.combine(&shape, m)),
        }
    }
}
