//! Geometric transforms of surfaces and whole documents.

use super::document::DocState;
use super::geom::Rect;
use super::selection::{Mask, Selection};
use super::surface::{unpremul, Surface};
use super::tiled::Tiled;
use rayon::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resample {
    Nearest,
    Bilinear,
    Bicubic,
    Supersample,
}

impl Resample {
    pub fn from_index(i: i32) -> Self {
        match i {
            0 => Resample::Supersample,
            1 => Resample::Bicubic,
            2 => Resample::Bilinear,
            _ => Resample::Nearest,
        }
    }
}

fn cubic(x: f32) -> f32 {
    let a = -0.5f32;
    let x = x.abs();
    if x <= 1.0 {
        (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0
    } else if x < 2.0 {
        a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a
    } else {
        0.0
    }
}

pub fn resize_surface(s: &Surface, nw: u32, nh: u32, mode: Resample) -> Surface {
    let mut out = Surface::new(nw, nh);
    let sx = s.w as f32 / nw as f32;
    let sy = s.h as f32 / nh as f32;
    let mode = if mode == Resample::Supersample && (sx < 1.0 || sy < 1.0) { Resample::Bicubic } else { mode };
    out.data.par_chunks_mut(nw as usize).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let fx = (x as f32 + 0.5) * sx;
            let fy = (y as f32 + 0.5) * sy;
            *o = match mode {
                Resample::Nearest => s.get_clamped(fx.floor() as i32, fy.floor() as i32),
                Resample::Bilinear => s.sample_bilinear_clamped(fx, fy),
                Resample::Bicubic => {
                    let cx = fx - 0.5;
                    let cy = fy - 0.5;
                    let ix = cx.floor() as i32;
                    let iy = cy.floor() as i32;
                    let mut acc = [0f32; 4];
                    let mut wsum = 0.0;
                    for j in -1..=2 {
                        let wy = cubic(cy - (iy + j) as f32);
                        for i in -1..=2 {
                            let wgt = cubic(cx - (ix + i) as f32) * wy;
                            let p = s.get_clamped(ix + i, iy + j);
                            let a = p[3] as f32 / 255.0;
                            acc[0] += p[0] as f32 * a * wgt;
                            acc[1] += p[1] as f32 * a * wgt;
                            acc[2] += p[2] as f32 * a * wgt;
                            acc[3] += p[3] as f32 * wgt;
                            wsum += wgt;
                        }
                    }
                    for v in acc.iter_mut() {
                        *v /= wsum;
                    }
                    acc[3] = acc[3].clamp(0.0, 255.0);
                    for i in 0..3 {
                        acc[i] = acc[i].clamp(0.0, acc[3]);
                    }
                    unpremul(acc)
                }
                Resample::Supersample => {
                    let x0 = x as f32 * sx;
                    let x1 = x0 + sx;
                    let y0 = y as f32 * sy;
                    let y1 = y0 + sy;
                    let mut acc = [0f32; 4];
                    let mut area = 0.0;
                    let mut yy = y0.floor() as i32;
                    while (yy as f32) < y1 {
                        let hy = (y1.min(yy as f32 + 1.0) - y0.max(yy as f32)).max(0.0);
                        let mut xx = x0.floor() as i32;
                        while (xx as f32) < x1 {
                            let wx = (x1.min(xx as f32 + 1.0) - x0.max(xx as f32)).max(0.0);
                            let wgt = wx * hy;
                            let p = s.get_clamped(xx, yy);
                            let a = p[3] as f32 / 255.0;
                            acc[0] += p[0] as f32 * a * wgt;
                            acc[1] += p[1] as f32 * a * wgt;
                            acc[2] += p[2] as f32 * a * wgt;
                            acc[3] += p[3] as f32 * wgt;
                            area += wgt;
                            xx += 1;
                        }
                        yy += 1;
                    }
                    for v in acc.iter_mut() {
                        *v /= area.max(1e-6);
                    }
                    unpremul(acc)
                }
            };
        }
    });
    out
}

fn map_layers(st: &mut DocState, nw: u32, nh: u32, f: impl Fn(&Surface) -> Surface) {
    for l in st.layers.iter_mut() {
        let s = l.px.to_surface();
        l.px = Tiled::from_surface(&f(&s));
    }
    st.w = nw;
    st.h = nh;
}

fn mask_to_surface(m: &Mask) -> Surface {
    Surface { w: m.w, h: m.h, data: m.data.iter().map(|v| [255, 255, 255, *v]).collect() }
}
fn surface_to_mask(s: &Surface) -> Mask {
    Mask::from_coverage(s.w, s.h, s.data.iter().map(|p| p[3]).collect())
}

fn map_selection(st: &mut DocState, f: impl Fn(&Surface) -> Surface) {
    if let Some(m) = &st.selection.mask {
        let s = f(&mask_to_surface(m));
        st.selection = Selection::from_mask(surface_to_mask(&s));
    }
}

pub fn resize_image(st: &mut DocState, nw: u32, nh: u32, mode: Resample) {
    map_layers(st, nw, nh, |s| resize_surface(s, nw, nh, mode));
    st.selection = Selection::none();
}

/// Anchor: 0..8 (row-major 3x3 grid).
pub fn canvas_size(st: &mut DocState, nw: u32, nh: u32, anchor: i32, fill_bottom: Option<[u8; 4]>) {
    let ax = anchor % 3;
    let ay = anchor / 3;
    let dx = match ax {
        0 => 0,
        1 => (nw as i32 - st.w as i32) / 2,
        _ => nw as i32 - st.w as i32,
    };
    let dy = match ay {
        0 => 0,
        1 => (nh as i32 - st.h as i32) / 2,
        _ => nh as i32 - st.h as i32,
    };
    let n_layers = st.layers.len();
    for (i, l) in st.layers.iter_mut().enumerate() {
        let s = l.px.to_surface();
        let bg = if i == 0 && n_layers > 0 { fill_bottom.unwrap_or([0; 4]) } else { [0; 4] };
        let mut out = Surface::filled(nw, nh, bg);
        for y in 0..s.h as i32 {
            for x in 0..s.w as i32 {
                if out.in_bounds(x + dx, y + dy) {
                    out.set(x + dx, y + dy, s.get(x, y));
                }
            }
        }
        l.px = Tiled::from_surface(&out);
    }
    st.w = nw;
    st.h = nh;
    st.selection = Selection::none();
}

pub fn flip_surface(s: &Surface, horizontal: bool) -> Surface {
    let mut out = Surface::new(s.w, s.h);
    for y in 0..s.h as i32 {
        for x in 0..s.w as i32 {
            let (sx, sy) = if horizontal { (s.w as i32 - 1 - x, y) } else { (x, s.h as i32 - 1 - y) };
            out.set(x, y, s.get(sx, sy));
        }
    }
    out
}

/// Rotates by multiples of 90° clockwise.
pub fn rotate90_surface(s: &Surface, quarter_turns: i32) -> Surface {
    let q = quarter_turns.rem_euclid(4);
    let (nw, nh) = if q % 2 == 1 { (s.h, s.w) } else { (s.w, s.h) };
    let mut out = Surface::new(nw, nh);
    for y in 0..nh as i32 {
        for x in 0..nw as i32 {
            let (sx, sy) = match q {
                0 => (x, y),
                1 => (y, s.h as i32 - 1 - x),
                2 => (s.w as i32 - 1 - x, s.h as i32 - 1 - y),
                _ => (s.w as i32 - 1 - y, x),
            };
            out.set(x, y, s.get(sx, sy));
        }
    }
    out
}

pub fn flip_image(st: &mut DocState, horizontal: bool) {
    let (w, h) = (st.w, st.h);
    map_layers(st, w, h, |s| flip_surface(s, horizontal));
    map_selection(st, |s| flip_surface(s, horizontal));
}

pub fn rotate_image(st: &mut DocState, quarter_turns: i32) {
    let (w, h) = if quarter_turns.rem_euclid(2) == 1 { (st.h, st.w) } else { (st.w, st.h) };
    map_layers(st, w, h, |s| rotate90_surface(s, quarter_turns));
    map_selection(st, |s| rotate90_surface(s, quarter_turns));
}

/// Crops to the selection bounds. Pixels outside a non-rectangular selection become transparent.
pub fn crop_to_selection(st: &mut DocState) {
    let Some(m) = st.selection.mask.clone() else { return };
    let r = m.bounds;
    if r.is_empty() {
        return;
    }
    let rect_sel = m.is_rectangular();
    for l in st.layers.iter_mut() {
        let mut s = l.px.read_rect(r);
        if !rect_sel {
            for y in 0..s.h as i32 {
                for x in 0..s.w as i32 {
                    let c = m.get(x + r.x0, y + r.y0);
                    if c < 255 {
                        let mut p = s.get(x, y);
                        p[3] = (p[3] as u16 * c as u16 / 255) as u8;
                        s.set(x, y, p);
                    }
                }
            }
        }
        l.px = Tiled::from_surface(&s);
    }
    st.w = r.width() as u32;
    st.h = r.height() as u32;
    st.selection = Selection::none();
}

pub fn crop_rect(st: &mut DocState, r: Rect) {
    for l in st.layers.iter_mut() {
        l.px = Tiled::from_surface(&l.px.read_rect(r));
    }
    st.w = r.width() as u32;
    st.h = r.height() as u32;
    st.selection = Selection::none();
}

/// Rotate, zoom and pan a surface around its center. Angle in degrees.
pub fn rotate_zoom(s: &Surface, angle_deg: f32, zoom: f32, pan_x: f32, pan_y: f32, tile: bool) -> Surface {
    let mut out = Surface::new(s.w, s.h);
    let (cx, cy) = (s.w as f32 / 2.0, s.h as f32 / 2.0);
    let a = -angle_deg.to_radians();
    let (sn, cs) = a.sin_cos();
    let z = zoom.max(0.01);
    out.data.par_chunks_mut(s.w as usize).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let px = x as f32 + 0.5 - cx - pan_x * s.w as f32;
            let py = y as f32 + 0.5 - cy - pan_y * s.h as f32;
            let mut sx = (px * cs - py * sn) / z + cx;
            let mut sy = (px * sn + py * cs) / z + cy;
            if tile {
                sx = sx.rem_euclid(s.w as f32);
                sy = sy.rem_euclid(s.h as f32);
            }
            *o = unpremul(s.sample_bilinear_premul(sx, sy));
        }
    });
    out
}

/// Bounds of opaque content across all layers (for Auto Crop).
pub fn content_bounds(st: &DocState) -> Rect {
    let flat = st.flatten();
    let corner = flat.get(0, 0);
    let mut r = Rect::EMPTY;
    for y in 0..flat.h as i32 {
        for x in 0..flat.w as i32 {
            if flat.get(x, y) != corner {
                r = r.union(&Rect::new(x, y, x + 1, y + 1));
            }
        }
    }
    r
}
