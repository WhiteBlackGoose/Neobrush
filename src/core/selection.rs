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

/// Coverage storage of a mask, limited to `Mask::region`.
#[derive(Clone, Debug)]
enum Store {
    /// Every pixel inside the region is fully selected.
    Solid,
    /// Row-major coverage for the region.
    Data(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Mask {
    pub w: u32,
    pub h: u32,
    /// Area that holds coverage data; everything outside is unselected.
    region: Rect,
    store: Store,
    /// Bounding box of pixels with coverage >= 128.
    pub bounds: Rect,
    pub outline: Vec<Seg>,
}

impl Mask {
    pub fn empty(w: u32, h: u32) -> Mask {
        Mask { w, h, region: Rect::EMPTY, store: Store::Data(vec![]), bounds: Rect::EMPTY, outline: vec![] }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        let r = &self.region;
        if x < r.x0 || y < r.y0 || x >= r.x1 || y >= r.y1 {
            return 0;
        }
        match &self.store {
            Store::Solid => 255,
            Store::Data(d) => d[((y - r.y0) * r.width() + (x - r.x0)) as usize],
        }
    }

    /// Builds a mask from coverage data covering `region` (clipped to the image).
    pub fn from_region(w: u32, h: u32, region: Rect, data: Vec<u8>) -> Mask {
        debug_assert_eq!(data.len(), (region.width() * region.height()).max(0) as usize);
        let m = Mask { w, h, region, store: Store::Data(data), bounds: Rect::EMPTY, outline: vec![] };
        m.finish()
    }

    /// Builds a mask from full-image coverage, keeping only the used area.
    pub fn from_coverage(w: u32, h: u32, data: Vec<u8>) -> Mask {
        let full = Rect::from_size(w, h);
        let mut b = Rect::EMPTY;
        for y in 0..h as i32 {
            let row = &data[(y as u32 * w) as usize..((y as u32 + 1) * w) as usize];
            if let Some(first) = row.iter().position(|v| *v != 0) {
                let last = row.iter().rposition(|v| *v != 0).unwrap();
                b = b.union(&Rect::new(first as i32, y, last as i32 + 1, y + 1));
            }
        }
        if b.is_empty() {
            return Mask::empty(w, h);
        }
        if b == full {
            return Mask::from_region(w, h, full, data);
        }
        let mut out = Vec::with_capacity((b.width() * b.height()) as usize);
        for y in b.y0..b.y1 {
            let start = (y as u32 * w + b.x0 as u32) as usize;
            out.extend_from_slice(&data[start..start + b.width() as usize]);
        }
        Mask::from_region(w, h, b, out)
    }

    /// Recomputes bounds and outline.
    fn finish(mut self) -> Mask {
        let r = self.region;
        if let Store::Solid = self.store {
            self.bounds = r;
            self.outline = if r.is_empty() {
                vec![]
            } else {
                vec![
                    Seg { x0: r.x0, y0: r.y0, x1: r.x1, y1: r.y0 },
                    Seg { x0: r.x0, y0: r.y1, x1: r.x1, y1: r.y1 },
                    Seg { x0: r.x0, y0: r.y0, x1: r.x0, y1: r.y1 },
                    Seg { x0: r.x1, y0: r.y0, x1: r.x1, y1: r.y1 },
                ]
            };
            return self;
        }
        let rw = r.width();
        let mut b = Rect::EMPTY;
        if let Store::Data(d) = &self.store {
            for y in r.y0..r.y1 {
                let row = &d[((y - r.y0) * rw) as usize..((y - r.y0 + 1) * rw) as usize];
                if let Some(first) = row.iter().position(|v| *v >= 128) {
                    let last = row.iter().rposition(|v| *v >= 128).unwrap();
                    b = b.union(&Rect::new(r.x0 + first as i32, y, r.x0 + last as i32 + 1, y + 1));
                }
            }
        }
        self.bounds = b;
        let mut segs = Vec::new();
        if !b.is_empty() {
            // Row-major passes over bands of rows, in parallel. Horizontal edges compare against
            // the previous row; vertical edges are runs of transitions merged within a band.
            let bw = b.width() as usize;
            let store = &self.store;
            let row_of = |y: i32, out: &mut Vec<bool>| {
                out.clear();
                if y < b.y0 || y >= b.y1 {
                    out.resize(bw, false);
                    return;
                }
                if let Store::Data(d) = store {
                    let start = ((y - r.y0) * rw + (b.x0 - r.x0)) as usize;
                    out.extend(d[start..start + bw].iter().map(|v| *v >= 128));
                }
            };
            const BAND: i32 = 64;
            let bands: Vec<i32> = (b.y0..=b.y1).step_by(BAND as usize).collect();
            let parts: Vec<Vec<Seg>> = bands
                .par_iter()
                .map(|&band_y| {
                    let end = (band_y + BAND).min(b.y1 + 1);
                    let mut segs = Vec::new();
                    let mut prev = Vec::with_capacity(bw);
                    let mut cur = Vec::with_capacity(bw);
                    row_of(band_y - 1, &mut prev);
                    let mut open_v: Vec<i32> = vec![-1; bw + 1];
                    for y in band_y..end {
                        row_of(y, &mut cur);
                        let mut run: Option<usize> = None;
                        for x in 0..=bw {
                            let edge = x < bw && cur[x] != prev[x];
                            match (edge, run) {
                                (true, None) => run = Some(x),
                                (false, Some(s0)) => {
                                    segs.push(Seg { x0: b.x0 + s0 as i32, y0: y, x1: b.x0 + x as i32, y1: y });
                                    run = None;
                                }
                                _ => {}
                            }
                        }
                        for x in 0..=bw {
                            let left = x > 0 && cur[x - 1];
                            let right = x < bw && cur[x];
                            let edge = y < b.y1 && left != right;
                            if edge && open_v[x] < 0 {
                                open_v[x] = y;
                            } else if !edge && open_v[x] >= 0 {
                                segs.push(Seg { x0: b.x0 + x as i32, y0: open_v[x], x1: b.x0 + x as i32, y1: y });
                                open_v[x] = -1;
                            }
                        }
                        std::mem::swap(&mut prev, &mut cur);
                    }
                    for (x, start) in open_v.iter().enumerate() {
                        if *start >= 0 {
                            segs.push(Seg { x0: b.x0 + x as i32, y0: *start, x1: b.x0 + x as i32, y1: end });
                        }
                    }
                    segs
                })
                .collect();
            segs = parts.concat();
        }
        self.outline = segs;
        self
    }

    pub fn rect(w: u32, h: u32, r: Rect) -> Mask {
        let r = r.intersect(&Rect::from_size(w, h));
        if r.is_empty() {
            return Mask::empty(w, h);
        }
        Mask { w, h, region: r, store: Store::Solid, bounds: Rect::EMPTY, outline: vec![] }.finish()
    }

    pub fn ellipse(w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Mask {
        let cx = (x0 + x1) / 2.0;
        let cy = (y0 + y1) / 2.0;
        let rx = ((x1 - x0) / 2.0).abs().max(0.01);
        let ry = ((y1 - y0) / 2.0).abs().max(0.01);
        let r = Rect::from_points(x0, y0, x1, y1).inflate(1).intersect(&Rect::from_size(w, h));
        if r.is_empty() {
            return Mask::empty(w, h);
        }
        let rw = r.width() as usize;
        let mut data = vec![0u8; rw * r.height() as usize];
        // Exact horizontal coverage of the ellipse span on 4 sub-rows per pixel row.
        data.par_chunks_mut(rw).enumerate().for_each(|(yi, row)| {
            let y = r.y0 + yi as i32;
            let mut acc = vec![0u16; rw];
            for sub in 0..4 {
                let sy = y as f32 + (sub as f32 + 0.5) / 4.0;
                let t = (sy - cy) / ry;
                if t.abs() >= 1.0 {
                    continue;
                }
                let half = rx * (1.0 - t * t).sqrt();
                let xa = (cx - half).max(r.x0 as f32);
                let xb = (cx + half).min(r.x1 as f32);
                if xb <= xa {
                    continue;
                }
                for x in xa.floor() as i32..(xb.ceil() as i32).min(r.x1) {
                    let cov = (x as f32 + 1.0).min(xb) - (x as f32).max(xa);
                    if cov > 0.0 {
                        acc[(x - r.x0) as usize] += (cov * 64.0) as u16;
                    }
                }
            }
            for (o, a) in row.iter_mut().zip(acc) {
                *o = (a as u32 * 255 / 256).min(255) as u8;
            }
        });
        Mask::from_region(w, h, r, data)
    }

    /// Polygon (lasso) mask, even-odd rule with 4x vertical supersampling.
    pub fn polygon(w: u32, h: u32, pts: &[Pt]) -> Mask {
        if pts.len() < 3 {
            return Mask::empty(w, h);
        }
        let mut r = Rect::EMPTY;
        for p in pts {
            r = r.union(&Rect::new(p.x.floor() as i32, p.y.floor() as i32, p.x.ceil() as i32 + 1, p.y.ceil() as i32 + 1));
        }
        let r = r.intersect(&Rect::from_size(w, h));
        if r.is_empty() {
            return Mask::empty(w, h);
        }
        let rw = r.width() as usize;
        let mut data = vec![0u8; rw * r.height() as usize];
        data.par_chunks_mut(rw).enumerate().for_each(|(yi, row)| {
            let y = r.y0 + yi as i32;
            let mut acc = vec![0u16; rw];
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
                    let xa = pair[0].max(r.x0 as f32);
                    let xb = pair[1].min(r.x1 as f32);
                    if xb <= xa {
                        continue;
                    }
                    let ia = xa.floor() as i32;
                    let ib = xb.ceil() as i32;
                    for x in ia..ib.min(r.x1) {
                        let l = (x as f32).max(xa);
                        let rr = (x as f32 + 1.0).min(xb);
                        if rr > l {
                            acc[(x - r.x0) as usize] += ((rr - l) * 64.0) as u16;
                        }
                    }
                }
            }
            for x in 0..rw {
                row[x] = (acc[x] as u32 * 255 / 256).min(255) as u8;
            }
        });
        Mask::from_region(w, h, r, data)
    }

    pub fn combine(&self, other: &Mask, mode: SelectionMode) -> Mask {
        let region = match mode {
            SelectionMode::Replace => return other.clone(),
            SelectionMode::Union | SelectionMode::Xor => self.region.union(&other.region),
            SelectionMode::Exclude => self.region,
            SelectionMode::Intersect => self.region.intersect(&other.region),
        };
        if region.is_empty() {
            return Mask::empty(self.w, self.h);
        }
        let rw = region.width() as usize;
        let mut data = vec![0u8; rw * region.height() as usize];
        data.par_chunks_mut(rw).enumerate().for_each(|(yi, row)| {
            let y = region.y0 + yi as i32;
            for (xi, out) in row.iter_mut().enumerate() {
                let x = region.x0 + xi as i32;
                let (a, b) = (self.get(x, y), other.get(x, y));
                *out = match mode {
                    SelectionMode::Replace => b,
                    SelectionMode::Union => a.max(b),
                    SelectionMode::Exclude => ((a as u16 * (255 - b) as u16) / 255) as u8,
                    SelectionMode::Intersect => ((a as u16 * b as u16) / 255) as u8,
                    SelectionMode::Xor => {
                        let (a, b) = (a as i32, b as i32);
                        (a + b - 2 * a * b / 255).clamp(0, 255) as u8
                    }
                };
            }
        });
        Mask::from_region(self.w, self.h, region, data)
    }

    pub fn invert(&self) -> Mask {
        let full = Rect::from_size(self.w, self.h);
        let mut data = vec![0u8; (self.w * self.h) as usize];
        data.par_chunks_mut(self.w as usize).enumerate().for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                *out = 255 - self.get(x as i32, y as i32);
            }
        });
        Mask::from_coverage(full.width() as u32, full.height() as u32, data)
    }

    /// True when the selection is exactly its bounding rectangle.
    pub fn is_rectangular(&self) -> bool {
        match &self.store {
            Store::Solid => true,
            Store::Data(_) => {
                let b = self.bounds;
                if b.is_empty() {
                    return false;
                }
                for y in self.region.y0..self.region.y1 {
                    for x in self.region.x0..self.region.x1 {
                        let v = self.get(x, y);
                        if b.contains(x, y) != (v == 255) || (v != 0 && v != 255) {
                            return false;
                        }
                    }
                }
                true
            }
        }
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
