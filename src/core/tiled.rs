//! Tiled, copy-on-write pixel storage. Cloning is cheap, which makes snapshot based undo cheap.

use super::geom::Rect;
use super::surface::{Px, Surface};
use rayon::prelude::*;
use std::sync::Arc;

pub const TS: i32 = 64;
const TN: usize = (TS * TS) as usize;

pub type Tile = [Px; TN];

#[derive(Clone)]
pub struct Tiled {
    pub w: u32,
    pub h: u32,
    tw: i32,
    /// `None` is a fully transparent tile.
    tiles: Vec<Option<Arc<Tile>>>,
}

impl Tiled {
    pub fn new(w: u32, h: u32) -> Self {
        let tw = (w as i32 + TS - 1) / TS;
        let th = (h as i32 + TS - 1) / TS;
        Tiled { w, h, tw, tiles: vec![None; (tw * th) as usize] }
    }

    pub fn filled(w: u32, h: u32, c: Px) -> Self {
        let mut t = Tiled::new(w, h);
        if c[3] != 0 {
            let tile = Arc::new([c; TN]);
            for slot in t.tiles.iter_mut() {
                *slot = Some(tile.clone());
            }
        }
        t
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Px {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return [0; 4];
        }
        match &self.tiles[((y / TS) * self.tw + x / TS) as usize] {
            None => [0; 4],
            Some(t) => t[((y % TS) * TS + x % TS) as usize],
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn set(&mut self, x: i32, y: i32, c: Px) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let slot = &mut self.tiles[((y / TS) * self.tw + x / TS) as usize];
        if slot.is_none() {
            if c[3] == 0 && c == [0; 4] {
                return;
            }
            *slot = Some(Arc::new([[0; 4]; TN]));
        }
        let t = Arc::make_mut(slot.as_mut().unwrap());
        t[((y % TS) * TS + x % TS) as usize] = c;
    }

    pub fn rect(&self) -> Rect {
        Rect::from_size(self.w, self.h)
    }

    /// Copies a row segment `[x0, x1)` at row `y` into `out` (which must be large enough).
    pub fn read_row(&self, y: i32, x0: i32, x1: i32, out: &mut [Px]) {
        let mut x = x0;
        let mut o = 0usize;
        while x < x1 {
            let tx = x / TS;
            let seg_end = ((tx + 1) * TS).min(x1);
            let n = (seg_end - x) as usize;
            match &self.tiles[((y / TS) * self.tw + tx) as usize] {
                None => out[o..o + n].fill([0; 4]),
                Some(t) => {
                    let base = ((y % TS) * TS + x % TS) as usize;
                    out[o..o + n].copy_from_slice(&t[base..base + n]);
                }
            }
            o += n;
            x = seg_end;
        }
    }

    pub fn to_surface(&self) -> Surface {
        let mut s = Surface::new(self.w, self.h);
        let w = self.w as usize;
        s.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            self.read_row(y as i32, 0, w as i32, row);
        });
        s
    }

    pub fn read_rect(&self, r: Rect) -> Surface {
        let r = r.intersect(&self.rect());
        let mut s = Surface::new(r.width() as u32, r.height() as u32);
        if r.is_empty() {
            return s;
        }
        let w = r.width() as usize;
        s.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            self.read_row(r.y0 + y as i32, r.x0, r.x1, row);
        });
        s
    }

    pub fn from_surface(s: &Surface) -> Tiled {
        let mut t = Tiled::new(s.w, s.h);
        t.update_from_surface(s);
        t
    }

    /// Replaces contents with `s` (same size), keeping tiles that did not change shared.
    pub fn update_from_surface(&mut self, s: &Surface) {
        assert!(s.w == self.w && s.h == self.h);
        let tw = self.tw;
        let (w, h) = (self.w as i32, self.h as i32);
        self.tiles.par_iter_mut().enumerate().for_each(|(i, slot)| {
            let tx = i as i32 % tw;
            let ty = i as i32 / tw;
            let mut tile = [[0u8; 4]; TN];
            let mut any = false;
            for yy in 0..TS {
                let y = ty * TS + yy;
                if y >= h {
                    break;
                }
                for xx in 0..TS {
                    let x = tx * TS + xx;
                    if x >= w {
                        break;
                    }
                    let p = s.data[(y * w + x) as usize];
                    if p != [0; 4] {
                        any = true;
                    }
                    tile[(yy * TS + xx) as usize] = p;
                }
            }
            match slot {
                Some(old) if any && **old == tile => {}
                _ => *slot = if any { Some(Arc::new(tile)) } else { None },
            }
        });
    }

    /// Applies `f` to every pixel in rect `r` in parallel (by tile).
    pub fn map_rect<F>(&mut self, r: Rect, f: F)
    where
        F: Fn(i32, i32, Px) -> Px + Sync,
    {
        let r = r.intersect(&self.rect());
        if r.is_empty() {
            return;
        }
        let tw = self.tw;
        self.tiles.par_iter_mut().enumerate().for_each(|(i, slot)| {
            let tx = i as i32 % tw;
            let ty = i as i32 / tw;
            let tr = Rect::new(tx * TS, ty * TS, tx * TS + TS, ty * TS + TS).intersect(&r);
            if tr.is_empty() {
                return;
            }
            let mut changed = false;
            let mut tile: Tile = match slot {
                None => [[0; 4]; TN],
                Some(t) => **t,
            };
            for y in tr.y0..tr.y1 {
                for x in tr.x0..tr.x1 {
                    let idx = ((y - ty * TS) * TS + (x - tx * TS)) as usize;
                    let o = tile[idx];
                    let n = f(x, y, o);
                    if n != o {
                        tile[idx] = n;
                        changed = true;
                    }
                }
            }
            if changed {
                if tile.iter().all(|p| *p == [0; 4]) {
                    *slot = None;
                } else {
                    *slot = Some(Arc::new(tile));
                }
            }
        });
    }
}
