//! Documents, layers and snapshot-based history.

use super::blend::BlendMode;
use super::geom::Rect;
use super::selection::Selection;
use super::surface::{Px, Surface};
use super::tiled::Tiled;
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    /// 0..1
    pub opacity: f32,
    pub blend: BlendMode,
    pub px: Tiled,
}

impl Layer {
    pub fn new(name: impl Into<String>, w: u32, h: u32) -> Self {
        Layer { name: name.into(), visible: true, opacity: 1.0, blend: BlendMode::Normal, px: Tiled::new(w, h) }
    }
    pub fn filled(name: impl Into<String>, w: u32, h: u32, c: Px) -> Self {
        Layer { px: Tiled::filled(w, h, c), ..Layer::new(name, w, h) }
    }
    pub fn from_surface(name: impl Into<String>, s: &Surface) -> Self {
        Layer { px: Tiled::from_surface(s), ..Layer::new(name, s.w, s.h) }
    }
}

#[derive(Clone)]
pub struct DocState {
    pub w: u32,
    pub h: u32,
    /// Bottom to top.
    pub layers: Vec<Layer>,
    pub active: usize,
    pub selection: Selection,
}

impl DocState {
    pub fn layer(&self) -> &Layer {
        &self.layers[self.active]
    }
    pub fn layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[self.active]
    }
    pub fn rect(&self) -> Rect {
        Rect::from_size(self.w, self.h)
    }

    /// Composites visible layers for the given rect into `out` (which is full document size).
    pub fn composite_into(&self, out: &mut Surface, r: Rect) {
        let r = r.intersect(&self.rect());
        if r.is_empty() {
            return;
        }
        let w = self.w as usize;
        let rw = r.width() as usize;
        let layers: Vec<&Layer> = self.layers.iter().filter(|l| l.visible && l.opacity > 0.0).collect();
        out.data
            .par_chunks_mut(w)
            .enumerate()
            .skip(r.y0 as usize)
            .take(r.height() as usize)
            .for_each(|(y, row)| {
                let mut acc = vec![[0f32; 4]; rw];
                let mut buf = vec![[0u8; 4]; rw];
                for l in &layers {
                    l.px.read_row(y as i32, r.x0, r.x1, &mut buf);
                    let normal = l.blend == BlendMode::Normal && l.opacity >= 1.0;
                    for (a, p) in acc.iter_mut().zip(buf.iter()) {
                        if p[3] == 0 {
                            continue;
                        }
                        if normal && p[3] == 255 {
                            *a = [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, 1.0];
                            continue;
                        }
                        let s = [
                            p[0] as f32 / 255.0,
                            p[1] as f32 / 255.0,
                            p[2] as f32 / 255.0,
                            p[3] as f32 / 255.0 * l.opacity,
                        ];
                        l.blend.composite(a, s);
                    }
                }
                for (i, a) in acc.iter().enumerate() {
                    let px = if a[3] <= 0.0 {
                        [0, 0, 0, 0]
                    } else {
                        [
                            (a[0] / a[3] * 255.0).round().clamp(0.0, 255.0) as u8,
                            (a[1] / a[3] * 255.0).round().clamp(0.0, 255.0) as u8,
                            (a[2] / a[3] * 255.0).round().clamp(0.0, 255.0) as u8,
                            (a[3] * 255.0).round().clamp(0.0, 255.0) as u8,
                        ]
                    };
                    row[r.x0 as usize + i] = px;
                }
            });
    }

    /// Area whose composite may differ between two states (None = everything).
    pub fn composite_diff(&self, other: &DocState) -> Option<Rect> {
        if self.w != other.w || self.h != other.h || self.layers.len() != other.layers.len() {
            return None;
        }
        let mut r = Rect::EMPTY;
        for (a, b) in self.layers.iter().zip(other.layers.iter()) {
            if a.visible != b.visible || a.opacity != b.opacity || a.blend != b.blend {
                if a.visible || b.visible {
                    return None;
                }
                continue;
            }
            if a.visible {
                r = r.union(&a.px.diff_rect(&b.px));
            }
        }
        Some(r)
    }

    pub fn flatten(&self) -> Surface {
        let mut s = Surface::new(self.w, self.h);
        self.composite_into(&mut s, self.rect());
        s
    }
}

pub struct HistEntry {
    pub name: String,
    pub icon: &'static str,
    pub state: DocState,
}

pub struct History {
    pub entries: Vec<HistEntry>,
    pub index: usize,
}

pub const HISTORY_LIMIT: usize = 150;

impl History {
    pub fn new(name: &str, icon: &'static str, state: DocState) -> Self {
        History { entries: vec![HistEntry { name: name.into(), icon, state }], index: 0 }
    }
    pub fn current(&self) -> &DocState {
        &self.entries[self.index].state
    }
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }
    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.entries.len()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub zoom: f32,
    /// Screen position (physical px) of document origin.
    pub ox: f32,
    pub oy: f32,
    /// Set until the first time the view has been fitted to a viewport.
    pub needs_fit: bool,
    /// Keep re-fitting on viewport resizes until the user zooms or pans.
    pub auto: bool,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: 1.0, ox: 0.0, oy: 0.0, needs_fit: true, auto: true }
    }
}

pub struct Document {
    pub id: u64,
    pub state: DocState,
    pub history: History,
    pub path: Option<PathBuf>,
    pub title: String,
    /// History index that matches what is on disk (None if never matches).
    pub saved_index: Option<usize>,
    pub composite: Surface,
    /// Downscaled copies of the composite (each half the size of the previous).
    pub mips: Vec<Surface>,
    pub dirty: Rect,
    /// Area changed since the viewport was last rendered.
    pub view_dirty: Rect,
    pub view: View,
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Document {
    pub fn new(title: String, state: DocState, hist_name: &str, saved: bool) -> Self {
        let mut composite = Surface::new(state.w, state.h);
        state.composite_into(&mut composite, state.rect());
        Document {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            history: History::new(hist_name, "image", state.clone()),
            state,
            path: None,
            title,
            saved_index: if saved { Some(0) } else { None },
            mips: build_mips(&composite),
            composite,
            dirty: Rect::EMPTY,
            view_dirty: Rect::EMPTY,
            view: View::default(),
        }
    }

    pub fn blank(title: String, w: u32, h: u32, bg: Px) -> Self {
        let layer = if bg[3] == 0 { Layer::new("Background", w, h) } else { Layer::filled("Background", w, h, bg) };
        let state = DocState { w, h, layers: vec![layer], active: 0, selection: Selection::none() };
        Document::new(title, state, "New Image", false)
    }

    pub fn is_modified(&self) -> bool {
        self.saved_index != Some(self.history.index)
    }

    pub fn invalidate(&mut self, r: Rect) {
        let r = r.intersect(&self.state.rect());
        self.dirty = self.dirty.union(&r);
    }
    pub fn invalidate_all(&mut self) {
        self.dirty = self.state.rect();
    }

    /// Recomposites dirty areas. Returns true if anything changed.
    pub fn update_composite(&mut self) -> bool {
        if self.composite.w != self.state.w || self.composite.h != self.state.h {
            self.composite = Surface::new(self.state.w, self.state.h);
            self.mips = build_mips(&self.composite);
            self.dirty = self.state.rect();
        }
        if self.dirty.is_empty() {
            return false;
        }
        let d = self.dirty;
        self.state.composite_into(&mut self.composite, d);
        update_mips(&self.composite, &mut self.mips, d);
        self.view_dirty = self.view_dirty.union(&d);
        self.dirty = Rect::EMPTY;
        true
    }

    /// Records the current working state as a new history step.
    pub fn commit(&mut self, name: &str, icon: &'static str) {
        let h = &mut self.history;
        h.entries.truncate(h.index + 1);
        if let Some(s) = self.saved_index {
            if s > h.index {
                self.saved_index = None;
            }
        }
        h.entries.push(HistEntry { name: name.into(), icon, state: self.state.clone() });
        h.index += 1;
        if h.entries.len() > HISTORY_LIMIT {
            h.entries.remove(0);
            h.index -= 1;
            self.saved_index = match self.saved_index {
                Some(0) | None => None,
                Some(s) => Some(s - 1),
            };
        }
    }

    /// Discards uncommitted changes of the working state.
    pub fn revert(&mut self) {
        let prev = self.history.current().clone();
        let diff = self.state.composite_diff(&prev);
        self.state = prev;
        match diff {
            Some(r) => self.invalidate(r),
            None => self.invalidate_all(),
        }
    }

    pub fn goto_history(&mut self, idx: usize) {
        if idx < self.history.entries.len() {
            self.history.index = idx;
            self.revert();
        }
    }
    pub fn undo(&mut self) {
        if self.history.can_undo() {
            self.goto_history(self.history.index - 1);
        }
    }
    pub fn redo(&mut self) {
        if self.history.can_redo() {
            self.goto_history(self.history.index + 1);
        }
    }
}

fn mip_sizes(w: u32, h: u32) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    let (mut w, mut h) = (w, h);
    while w > 64 || h > 64 {
        w = w.div_ceil(2).max(1);
        h = h.div_ceil(2).max(1);
        v.push((w, h));
    }
    v
}

fn build_mips(base: &Surface) -> Vec<Surface> {
    let mut mips: Vec<Surface> = mip_sizes(base.w, base.h).into_iter().map(|(w, h)| Surface::new(w, h)).collect();
    update_mips(base, &mut mips, Rect::from_size(base.w, base.h));
    mips
}

/// Recomputes the mip levels covering `r` (in base coordinates) with a premultiplied 2x2 box filter.
fn update_mips(base: &Surface, mips: &mut [Surface], r: Rect) {
    let mut r = r;
    for i in 0..mips.len() {
        let (prev, rest) = mips.split_at_mut(i);
        let src: &Surface = if i == 0 { base } else { &prev[i - 1] };
        let dst = &mut rest[0];
        r = Rect::new(r.x0.div_euclid(2), r.y0.div_euclid(2), (r.x1 + 1) / 2, (r.y1 + 1) / 2).intersect(&Rect::from_size(dst.w, dst.h));
        if r.is_empty() {
            return;
        }
        let dw = dst.w as usize;
        dst.data
            .par_chunks_mut(dw)
            .enumerate()
            .skip(r.y0 as usize)
            .take(r.height() as usize)
            .for_each(|(y, row)| {
                let y = y as i32;
                for x in r.x0..r.x1 {
                    let mut acc = [0u32; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = src.get_clamped(x * 2 + dx, y * 2 + dy);
                        let a = p[3] as u32;
                        acc[0] += p[0] as u32 * a;
                        acc[1] += p[1] as u32 * a;
                        acc[2] += p[2] as u32 * a;
                        acc[3] += a;
                    }
                    row[x as usize] = if acc[3] == 0 {
                        [0; 4]
                    } else {
                        [(acc[0] / acc[3]) as u8, (acc[1] / acc[3]) as u8, (acc[2] / acc[3]) as u8, (acc[3] / 4) as u8]
                    };
                }
            });
    }
}
