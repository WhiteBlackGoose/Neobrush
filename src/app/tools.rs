//! Pointer and keyboard handling for the canvas tools.

use super::editor::{color_to_px, px_to_color, Editor};
use super::session::*;
use crate::core::geom::{Pt, Rect};
use crate::core::paint::{self, Cov, GradientKind, PaintOp, SHAPES};
use crate::core::selection::{Mask, Selection, SelectionMode};
use crate::core::surface::Surface;
use crate::core::text::TextStyle;
use crate::{App, Tool};
use slint::ComponentHandle;
use slint::platform::Key;

const DOWN: i32 = 0;
const UP: i32 = 1;
const MOVE: i32 = 2;
const CANCEL: i32 = 3;
const LEAVE: i32 = 4;

fn key_str(k: Key) -> String {
    let c: char = k.into();
    c.to_string()
}

impl Editor {
    pub fn opts(&self) -> Opts {
        let ui = self.ui();
        let g = ui.global::<App>();
        Opts {
            primary: color_to_px(g.get_primary()),
            secondary: color_to_px(g.get_secondary()),
            width: g.get_brush_width() as f32,
            hardness: g.get_hardness() as f32 / 100.0,
            aa: g.get_antialias(),
            overwrite: g.get_overwrite(),
            tolerance: g.get_tolerance() as f32 / 100.0,
            global: g.get_flood_global(),
            sample_image: g.get_sample_image(),
            sel_mode: SelectionMode::from_index(g.get_sel_mode()),
            gradient: GradientKind::from_index(g.get_gradient_type()),
            gradient_alpha: g.get_gradient_alpha(),
            shape: SHAPES[(g.get_shape_type().max(0) as usize).min(SHAPES.len() - 1)],
            fill_style: g.get_fill_style(),
            radius: g.get_corner_radius() as f32,
            dash: g.get_dash(),
            text: TextStyle {
                family: g.get_font_family().to_string(),
                size: g.get_font_size() as f32,
                bold: g.get_bold(),
                italic: g.get_italic(),
                underline: g.get_underline(),
                align: g.get_text_align(),
                aa: g.get_antialias(),
            },
        }
    }

    pub fn select_tool(&mut self, t: Tool) {
        let prev = self.tool();
        if prev != t {
            self.finish_session(true);
            if t == Tool::Picker {
                self.picker_prev = Some(prev);
            }
        }
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_tool(t);
        g.set_status_hint(self.status_hint(t).into());
        self.update_cursor();
        self.refocus();
        self.redraw();
    }

    /// Re-renders an active editing session after options/colors changed.
    pub fn refresh_session(&mut self) {
        let o = self.opts();
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        match &mut self.session {
            Session::Shape(s) => s.render(doc, &o),
            Session::Line(l) => l.render(doc, &o),
            Session::Text(t) => t.render(doc, &o, &mut self.fonts),
            _ => {}
        }
    }

    /// Ends the current editing session. `commit` records it in the history.
    pub fn finish_session(&mut self, commit: bool) {
        let s = std::mem::replace(&mut self.session, Session::None);
        let primary = self.primary();
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        let mut used = false;
        match s {
            Session::None | Session::Move(_) => {}
            Session::Shape(sh) => {
                if commit && (sh.a.dist(sh.b) >= 1.0) {
                    doc.commit("Shape", "shapes");
                    used = true;
                } else {
                    doc.state.layers[sh.layer].px = sh.base;
                    doc.invalidate_all();
                }
            }
            Session::Line(l) => {
                if commit && l.pts[0].dist(l.pts[3]) >= 1.0 {
                    doc.commit("Line / Curve", "spline");
                    used = true;
                } else {
                    doc.state.layers[l.layer].px = l.base;
                    doc.invalidate_all();
                }
            }
            Session::Text(t) => {
                if commit && !t.text.trim().is_empty() {
                    doc.commit("Text", "type");
                    used = true;
                } else {
                    doc.state.layers[t.layer].px = t.base;
                    doc.invalidate_all();
                }
            }
        }
        if used {
            self.push_recent(primary);
        }
        self.panels();
    }

    fn tol_doc(&self) -> f32 {
        let z = self.doc().map(|d| d.view.zoom).unwrap_or(1.0);
        9.0 * self.scale / z
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pointer(&mut self, kind: i32, button: i32, lx: f32, ly: f32, ctrl: bool, shift: bool, alt: bool) {
        if self.doc().is_none() {
            return;
        }
        if kind == LEAVE {
            self.hover = None;
            self.redraw();
            return;
        }
        let p = self.to_doc(lx, ly);
        self.hover = Some(p);
        let sx = lx * self.scale;
        let sy = ly * self.scale;

        // Panning (middle button, Space, or Pan tool) takes priority.
        if let Drag::Pan { sx: psx, sy: psy, ox, oy } = self.drag {
            if kind == MOVE {
                let d = self.doc_mut().unwrap();
                d.view.ox = ox + (sx - psx);
                d.view.oy = oy + (sy - psy);
                self.clamp_view();
                self.redraw();
            } else if kind == UP || kind == CANCEL {
                self.drag = Drag::None;
                self.update_cursor();
            }
            return;
        }
        let tool = self.tool();
        if kind == DOWN && (button == 2 || (button == 0 && (self.space || tool == Tool::Pan))) {
            let v = self.doc().unwrap().view;
            self.drag = Drag::Pan { sx, sy, ox: v.ox, oy: v.oy };
            self.update_cursor();
            return;
        }
        if kind == DOWN && button > 1 {
            return;
        }

        match tool {
            Tool::Brush | Tool::Eraser | Tool::Pencil | Tool::Clone | Tool::Recolor => self.tool_stroke(tool, kind, button, p, ctrl, shift),
            Tool::RectSelect | Tool::EllipseSelect => self.tool_select(tool, kind, button, p, ctrl, shift, alt),
            Tool::LassoSelect => self.tool_lasso(kind, button, p, ctrl, alt),
            Tool::MagicWand => {
                if kind == DOWN {
                    self.tool_wand(button, p, ctrl, alt);
                }
            }
            Tool::MovePixels | Tool::MoveSelection => self.tool_move(tool, kind, button, p, shift),
            Tool::Zoom => self.tool_zoom(kind, button, p, sx, sy),
            Tool::Bucket => {
                if kind == DOWN {
                    self.tool_bucket(button, p);
                }
            }
            Tool::Gradient => self.tool_gradient(kind, button, p, shift),
            Tool::Picker => self.tool_picker(kind, button, p),
            Tool::Text => self.tool_text(kind, p),
            Tool::Line => self.tool_line(kind, button, p, shift),
            Tool::Shape => self.tool_shape(kind, button, p, shift),
            Tool::Pan => {}
        }
        if kind == MOVE {
            self.update_cursor();
            self.redraw();
        }
    }

    // ------------------------------------------------------------------------------------------
    // Painting strokes

    fn tool_stroke(&mut self, tool: Tool, kind: i32, button: i32, p: Pt, ctrl: bool, shift: bool) {
        match kind {
            DOWN => {
                if tool == Tool::Clone && ctrl {
                    self.clone_src = Some(p);
                    self.clone_offset = None;
                    self.redraw();
                    return;
                }
                if tool == Tool::Clone && self.clone_src.is_none() {
                    self.ui().global::<App>().set_status_hint("Ctrl+click to set the clone source first.".into());
                    return;
                }
                let o = self.opts();
                let doc = &self.docs[self.cur];
                let base = doc.state.layer().px.clone();
                let (c, c2) = if button == 1 { (o.secondary, o.primary) } else { (o.primary, o.secondary) };
                let (mut cdx, mut cdy) = (0, 0);
                let (op, name, icon) = match tool {
                    Tool::Brush => (PaintOp::Color { c, overwrite: o.overwrite }, "Paintbrush", "brush"),
                    Tool::Pencil => (PaintOp::Color { c, overwrite: o.overwrite }, "Pencil", "pencil"),
                    Tool::Eraser => (PaintOp::Erase, "Eraser", "eraser"),
                    Tool::Clone => {
                        let (dx, dy) = self.clone_offset.unwrap_or_else(|| {
                            let s = self.clone_src.unwrap();
                            ((s.x - p.x).round() as i32, (s.y - p.y).round() as i32)
                        });
                        cdx = dx;
                        cdy = dy;
                        (PaintOp::Clone { src: base.clone(), dx, dy }, "Clone Stamp", "stamp")
                    }
                    _ => (PaintOp::Recolor { from: c2, to: c, tol: o.tolerance.max(0.01) }, "Recolor", "replace"),
                };
                if tool == Tool::Clone {
                    self.clone_offset = Some((cdx, cdy));
                }
                let start = if shift { self.last_stroke_end.unwrap_or(p) } else { p };
                let mut st = Box::new(StrokeState {
                    base,
                    cov: Cov::new(doc.state.rect()),
                    last: start,
                    op,
                    width: if tool == Tool::Pencil { 1.0 } else { o.width },
                    hardness: o.hardness,
                    aa: o.aa,
                    pencil: tool == Tool::Pencil,
                    name,
                    icon,
                    clone_dx: cdx,
                    clone_dy: cdy,
                    color: c,
                });
                self.stroke_to(&mut st, p);
                self.drag = Drag::Stroke(st);
            }
            MOVE => {
                if let Drag::Stroke(mut st) = std::mem::replace(&mut self.drag, Drag::None) {
                    if st.last.dist(p) >= 0.35 {
                        self.stroke_to(&mut st, p);
                    }
                    self.drag = Drag::Stroke(st);
                }
            }
            UP | CANCEL => {
                if let Drag::Stroke(st) = std::mem::replace(&mut self.drag, Drag::None) {
                    let doc = &mut self.docs[self.cur];
                    doc.commit(st.name, st.icon);
                    self.last_stroke_end = Some(st.last);
                    if matches!(st.op, PaintOp::Color { .. }) {
                        self.push_recent(st.color);
                    }
                    self.panels();
                }
            }
            _ => {}
        }
    }

    fn stroke_to(&mut self, st: &mut StrokeState, p: Pt) {
        let doc = &mut self.docs[self.cur];
        let r = if st.pencil {
            paint::pencil_line(&mut st.cov, st.last, p)
        } else {
            paint::stroke_segment(&mut st.cov, st.last, p, st.width, st.hardness, st.aa)
        };
        let sel = doc.state.selection.clone();
        let layer = &mut doc.state.layers[doc.state.active];
        paint::apply_ops(&mut layer.px, &st.base, r, &sel, &[(&st.cov, &st.op)]);
        doc.invalidate(r);
        st.last = p;
        self.redraw();
    }

    // ------------------------------------------------------------------------------------------
    // Selection tools

    fn sel_mode(&self, button: i32, ctrl: bool, alt: bool) -> SelectionMode {
        let o = self.opts();
        if ctrl && button == 1 {
            SelectionMode::Xor
        } else if ctrl {
            SelectionMode::Union
        } else if alt || button == 1 {
            SelectionMode::Exclude
        } else {
            o.sel_mode
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tool_select(&mut self, tool: Tool, kind: i32, button: i32, p: Pt, ctrl: bool, shift: bool, alt: bool) {
        match kind {
            DOWN => {
                let mode = self.sel_mode(button, ctrl, alt);
                let base = self.docs[self.cur].state.selection.clone();
                self.drag = Drag::Select { start: p, base, mode, ellipse: tool == Tool::EllipseSelect, moved: false };
            }
            MOVE => {
                let tol = 2.0 * self.scale / self.doc().unwrap().view.zoom;
                if let Drag::Select { start, base, mode, ellipse, moved } = &mut self.drag {
                    if !*moved && start.dist(p) < tol {
                        return;
                    }
                    *moved = true;
                    let mut b = p;
                    if shift {
                        let d = (p.x - start.x).abs().max((p.y - start.y).abs());
                        b = Pt::new(start.x + d * (p.x - start.x).signum(), start.y + d * (p.y - start.y).signum());
                    }
                    let doc = &mut self.docs[self.cur];
                    let (w, h) = (doc.state.w, doc.state.h);
                    let m = if *ellipse {
                        Mask::ellipse(w, h, start.x.round(), start.y.round(), b.x.round(), b.y.round())
                    } else {
                        let r = Rect::new(start.x.min(b.x).round() as i32, start.y.min(b.y).round() as i32, start.x.max(b.x).round() as i32, start.y.max(b.y).round() as i32);
                        Mask::rect(w, h, r)
                    };
                    doc.state.selection = base.apply(m, *mode);
                    self.redraw();
                }
            }
            UP | CANCEL => {
                if let Drag::Select { base, moved, mode, ellipse, .. } = std::mem::replace(&mut self.drag, Drag::None) {
                    let doc = &mut self.docs[self.cur];
                    if !moved {
                        if mode == SelectionMode::Replace && base.is_active() {
                            doc.state.selection = Selection::none();
                            doc.commit("Deselect", "x");
                        }
                    } else {
                        doc.commit(if ellipse { "Ellipse Select" } else { "Rectangle Select" }, if ellipse { "ellipse-select" } else { "select" });
                    }
                    self.panels();
                }
            }
            _ => {}
        }
    }

    fn tool_lasso(&mut self, kind: i32, button: i32, p: Pt, ctrl: bool, alt: bool) {
        match kind {
            DOWN => {
                let mode = self.sel_mode(button, ctrl, alt);
                let base = self.docs[self.cur].state.selection.clone();
                self.drag = Drag::Lasso { pts: vec![p], base, mode };
            }
            MOVE => {
                if let Drag::Lasso { pts, base, mode } = &mut self.drag {
                    if pts.last().map(|l| l.dist(p) >= 0.75).unwrap_or(true) {
                        pts.push(p);
                        let doc = &mut self.docs[self.cur];
                        if pts.len() >= 3 {
                            let m = Mask::polygon(doc.state.w, doc.state.h, pts);
                            doc.state.selection = base.apply(m, *mode);
                        }
                        self.redraw();
                    }
                }
            }
            UP | CANCEL => {
                if let Drag::Lasso { pts, base, mode } = std::mem::replace(&mut self.drag, Drag::None) {
                    let doc = &mut self.docs[self.cur];
                    if pts.len() >= 3 {
                        doc.commit("Lasso Select", "lasso");
                    } else if mode == SelectionMode::Replace && base.is_active() {
                        doc.state.selection = Selection::none();
                        doc.commit("Deselect", "x");
                    }
                    self.panels();
                }
            }
            _ => {}
        }
    }

    fn sample_surface(&self, image: bool) -> Surface {
        let doc = &self.docs[self.cur];
        if image {
            doc.composite.clone()
        } else {
            doc.state.layer().px.to_surface()
        }
    }

    fn tool_wand(&mut self, button: i32, p: Pt, ctrl: bool, alt: bool) {
        let o = self.opts();
        let mode = self.sel_mode(button, ctrl, alt);
        let doc = &self.docs[self.cur];
        if !doc.state.rect().contains(p.x as i32, p.y as i32) {
            return;
        }
        let src = self.sample_surface(o.sample_image);
        let mask = paint::flood_mask(&src, p.x as i32, p.y as i32, o.tolerance * o.tolerance, !o.global);
        let doc = &mut self.docs[self.cur];
        let m = Mask::from_coverage(doc.state.w, doc.state.h, mask);
        doc.state.selection = doc.state.selection.apply(m, mode);
        doc.commit("Magic Wand Select", "wand");
        self.panels();
    }

    // ------------------------------------------------------------------------------------------
    // Move tools

    pub fn start_move_session(&mut self, pixels: bool) {
        let doc = &mut self.docs[self.cur];
        let (w, h) = (doc.state.w, doc.state.h);
        let mask = match &doc.state.selection.mask {
            Some(m) => m.clone(),
            None => std::sync::Arc::new(Mask::rect(w, h, doc.state.rect())),
        };
        let rect = mask.bounds;
        let layer = doc.state.active;
        let (cleared, float) = if pixels {
            let src = &doc.state.layers[layer].px;
            let mut float = src.read_rect(rect);
            for y in 0..float.h as i32 {
                for x in 0..float.w as i32 {
                    let c = mask.get(x + rect.x0, y + rect.y0);
                    if c < 255 {
                        let mut q = float.get(x, y);
                        q[3] = (q[3] as u16 * c as u16 / 255) as u8;
                        float.set(x, y, q);
                    }
                }
            }
            let mut cleared = src.clone();
            let m2 = mask.clone();
            cleared.map_rect(rect, |x, y, q| {
                let c = m2.get(x, y);
                if c == 0 {
                    q
                } else {
                    let a = (q[3] as u16 * (255 - c) as u16 / 255) as u8;
                    if a == 0 {
                        [0; 4]
                    } else {
                        [q[0], q[1], q[2], a]
                    }
                }
            });
            (cleared, float)
        } else {
            (doc.state.layers[layer].px.clone(), Surface::new(0, 0))
        };
        self.session = Session::Move(Box::new(MoveSession { pixels, layer, cleared, float, rect, mask, xf: Xf::default(), drag: None }));
    }

    fn tool_move(&mut self, tool: Tool, kind: i32, button: i32, p: Pt, shift: bool) {
        let pixels = tool == Tool::MovePixels;
        match kind {
            DOWN => {
                let valid = matches!(&self.session, Session::Move(m) if m.pixels == pixels && m.layer == self.docs[self.cur].state.active);
                if !valid {
                    self.finish_session(true);
                    self.start_move_session(pixels);
                }
                let tol = self.tol_doc();
                if let Session::Move(m) = &mut self.session {
                    let orig = m.xf;
                    m.drag = Some(if button == 1 {
                        let c = m.center();
                        let cc = Pt::new(c.x + orig.tx, c.y + orig.ty);
                        MoveDrag::Rotate { start: (p.y - cc.y).atan2(p.x - cc.x), orig }
                    } else if let Some(k) = m.hit_corner(p, tol) {
                        MoveDrag::Scale { corner: k, orig }
                    } else {
                        MoveDrag::Translate { start: p, orig }
                    });
                }
            }
            MOVE => {
                if let Session::Move(m) = &mut self.session {
                    if m.drag.is_some() {
                        m.drag_to(p, shift);
                        m.apply(&mut self.docs[self.cur]);
                        self.redraw();
                    }
                }
            }
            UP | CANCEL => {
                if let Session::Move(m) = &mut self.session {
                    if m.drag.take().is_some() {
                        let doc = &mut self.docs[self.cur];
                        let changed = {
                            let h = doc.history.current();
                            let a = h.selection.mask.as_ref().map(|m| m.bounds);
                            let b = doc.state.selection.mask.as_ref().map(|m| m.bounds);
                            a != b || m.xf.angle != 0.0 || m.xf.sx != 1.0 || m.xf.sy != 1.0
                        };
                        if changed {
                            if pixels {
                                doc.commit("Move Selected Pixels", "move");
                            } else {
                                doc.commit("Move Selection", "pointer");
                            }
                        }
                        self.panels();
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------------------------------
    // Zoom, bucket, gradient, picker

    fn tool_zoom(&mut self, kind: i32, button: i32, p: Pt, sx: f32, sy: f32) {
        match kind {
            DOWN => self.drag = Drag::ZoomRect { a: p, b: p, button },
            MOVE => {
                if let Drag::ZoomRect { b, .. } = &mut self.drag {
                    *b = p;
                    self.redraw();
                }
            }
            UP => {
                if let Drag::ZoomRect { a, b, button } = std::mem::replace(&mut self.drag, Drag::None) {
                    let z = self.doc().unwrap().view.zoom;
                    if (a.x - b.x).abs() * z < 6.0 && (a.y - b.y).abs() * z < 6.0 {
                        self.zoom_step(if button == 1 { -1 } else { 1 }, Some((sx, sy)));
                    } else {
                        let (cw, ch) = self.canvas_px();
                        let w = (a.x - b.x).abs().max(1.0);
                        let h = (a.y - b.y).abs().max(1.0);
                        let nz = (cw as f32 / w).min(ch as f32 / h).clamp(0.01, 64.0);
                        let d = self.doc_mut().unwrap();
                        d.view.zoom = nz;
                        d.view.ox = cw as f32 / 2.0 - (a.x + b.x) / 2.0 * nz;
                        d.view.oy = ch as f32 / 2.0 - (a.y + b.y) / 2.0 * nz;
                        self.clamp_view();
                    }
                    self.redraw();
                }
            }
            _ => self.drag = Drag::None,
        }
    }

    fn tool_bucket(&mut self, button: i32, p: Pt) {
        let o = self.opts();
        let doc = &self.docs[self.cur];
        if !doc.state.rect().contains(p.x as i32, p.y as i32) {
            return;
        }
        let src = self.sample_surface(o.sample_image);
        let mask = paint::flood_mask(&src, p.x as i32, p.y as i32, o.tolerance * o.tolerance, !o.global);
        let doc = &mut self.docs[self.cur];
        let r = doc.state.rect();
        let cov = Cov { rect: r, data: mask, touched: r };
        let c = if button == 1 { o.secondary } else { o.primary };
        let op = PaintOp::Color { c, overwrite: o.overwrite };
        let base = doc.state.layer().px.clone();
        let sel = doc.state.selection.clone();
        paint::apply_ops(&mut doc.state.layer_mut().px, &base, sel.bounds(r.x1 as u32, r.y1 as u32), &sel, &[(&cov, &op)]);
        doc.invalidate_all();
        doc.commit("Paint Bucket", "bucket");
        self.push_recent(c);
        self.panels();
    }

    fn tool_gradient(&mut self, kind: i32, button: i32, p: Pt, shift: bool) {
        match kind {
            DOWN => {
                let base = self.docs[self.cur].state.layer().px.clone();
                self.drag = Drag::Gradient { a: p, b: p, base, swap: button == 1 };
            }
            MOVE => {
                let o = self.opts();
                if let Drag::Gradient { a, b, base, swap } = &mut self.drag {
                    let mut q = p;
                    if shift {
                        let ang = (p.y - a.y).atan2(p.x - a.x);
                        let step = 15f32.to_radians();
                        let ang = (ang / step).round() * step;
                        let len = a.dist(p);
                        q = Pt::new(a.x + ang.cos() * len, a.y + ang.sin() * len);
                    }
                    *b = q;
                    let (c0, c1) = if *swap { (o.secondary, o.primary) } else { (o.primary, o.secondary) };
                    let doc = &mut self.docs[self.cur];
                    let sel = doc.state.selection.clone();
                    let active = doc.state.active;
                    paint::apply_gradient(&mut doc.state.layers[active].px, base, &sel, o.gradient, *a, *b, c0, c1, o.gradient_alpha, o.overwrite);
                    doc.invalidate_all();
                    self.redraw();
                }
            }
            UP | CANCEL => {
                if let Drag::Gradient { a, b, .. } = std::mem::replace(&mut self.drag, Drag::None) {
                    if a.dist(b) >= 1.0 {
                        self.docs[self.cur].commit("Gradient", "gradient");
                    }
                    self.panels();
                }
            }
            _ => {}
        }
    }

    fn tool_picker(&mut self, kind: i32, button: i32, p: Pt) {
        match kind {
            DOWN => {
                self.drag = Drag::Picker { button };
                self.pick_at(button, p);
            }
            MOVE => {
                if let Drag::Picker { button } = self.drag {
                    self.pick_at(button, p);
                }
            }
            UP => {
                if let Drag::Picker { .. } = std::mem::replace(&mut self.drag, Drag::None) {
                    let after = self.ui().global::<App>().get_picker_after();
                    match after {
                        1 => {
                            if let Some(t) = self.picker_prev {
                                self.select_tool(t);
                            }
                        }
                        2 => self.select_tool(Tool::Pencil),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn pick_at(&mut self, button: i32, p: Pt) {
        let ui = self.ui();
        let g = ui.global::<App>();
        let r = [0, 1, 2, 4][g.get_picker_size().clamp(0, 3) as usize];
        let image = g.get_sample_image();
        let doc = &self.docs[self.cur];
        let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
        if !doc.state.rect().contains(x, y) {
            return;
        }
        let mut acc = [0f32; 4];
        let mut n = 0.0;
        for dy in -r..=r {
            for dx in -r..=r {
                let (qx, qy) = (x + dx, y + dy);
                if !doc.state.rect().contains(qx, qy) {
                    continue;
                }
                let q = if image { doc.composite.get(qx, qy) } else { doc.state.layer().px.get(qx, qy) };
                let a = q[3] as f32 / 255.0;
                acc[0] += q[0] as f32 * a;
                acc[1] += q[1] as f32 * a;
                acc[2] += q[2] as f32 * a;
                acc[3] += q[3] as f32;
                n += 1.0;
            }
        }
        let c = if acc[3] > 0.0 {
            let a = acc[3] / 255.0;
            [(acc[0] / a) as u8, (acc[1] / a) as u8, (acc[2] / a) as u8, (acc[3] / n) as u8]
        } else {
            [0, 0, 0, 0]
        };
        if button == 1 {
            g.set_secondary(px_to_color(c));
        } else {
            g.set_primary(px_to_color(c));
        }
    }

    // ------------------------------------------------------------------------------------------
    // Text, line and shape sessions

    fn tool_text(&mut self, kind: i32, p: Pt) {
        match kind {
            DOWN => {
                if let Session::Text(t) = &mut self.session {
                    if t.bounds.inflate(3).contains(p.x as i32, p.y as i32) {
                        t.drag = Some((p, t.pos));
                        return;
                    }
                }
                self.finish_session(true);
                let doc = &self.docs[self.cur];
                self.session = Session::Text(Box::new(TextSession {
                    layer: doc.state.active,
                    base: doc.state.layer().px.clone(),
                    pos: Pt::new(p.x.round(), p.y.round()),
                    text: String::new(),
                    last: Rect::EMPTY,
                    caret: Rect::EMPTY,
                    bounds: Rect::EMPTY,
                    drag: None,
                }));
                self.caret_on = true;
                self.refresh_session();
                self.panels();
            }
            MOVE => {
                if let Session::Text(t) = &mut self.session {
                    if let Some((start, pos0)) = t.drag {
                        t.pos = Pt::new((pos0.x + p.x - start.x).round(), (pos0.y + p.y - start.y).round());
                        self.refresh_session();
                    }
                }
            }
            UP | CANCEL => {
                if let Session::Text(t) = &mut self.session {
                    t.drag = None;
                }
            }
            _ => {}
        }
    }

    fn tool_line(&mut self, kind: i32, button: i32, p: Pt, shift: bool) {
        match kind {
            DOWN => {
                let tol = self.tol_doc();
                if let Session::Line(l) = &mut self.session {
                    if let Some(i) = l.pts.iter().position(|q| q.dist(p) <= tol) {
                        l.drag = Some(i);
                        return;
                    }
                }
                self.finish_session(true);
                let doc = &self.docs[self.cur];
                self.session = Session::Line(Box::new(LineSession {
                    layer: doc.state.active,
                    base: doc.state.layer().px.clone(),
                    pts: [p; 4],
                    drag: Some(3),
                    creating: true,
                    swap: button == 1,
                    last: Rect::EMPTY,
                }));
            }
            MOVE => {
                let mut changed = false;
                if let Session::Line(l) = &mut self.session {
                    if let Some(i) = l.drag {
                        let mut q = p;
                        if shift && (i == 0 || i == 3) {
                            let o = if i == 3 { l.pts[0] } else { l.pts[3] };
                            let ang = (p.y - o.y).atan2(p.x - o.x);
                            let step = 15f32.to_radians();
                            let ang = (ang / step).round() * step;
                            let len = o.dist(p);
                            q = Pt::new(o.x + ang.cos() * len, o.y + ang.sin() * len);
                        }
                        l.pts[i] = q;
                        if l.creating {
                            l.pts[1] = l.pts[0].lerp(l.pts[3], 1.0 / 3.0);
                            l.pts[2] = l.pts[0].lerp(l.pts[3], 2.0 / 3.0);
                        }
                        changed = true;
                    }
                }
                if changed {
                    self.refresh_session();
                }
            }
            UP | CANCEL => {
                let mut cancel = false;
                if let Session::Line(l) = &mut self.session {
                    l.drag = None;
                    if l.creating && l.pts[0].dist(l.pts[3]) < 1.0 {
                        cancel = true;
                    }
                    l.creating = false;
                }
                if cancel {
                    self.finish_session(false);
                }
                self.panels();
            }
            _ => {}
        }
    }

    fn tool_shape(&mut self, kind: i32, button: i32, p: Pt, shift: bool) {
        match kind {
            DOWN => {
                let tol = self.tol_doc();
                if let Session::Shape(s) = &mut self.session {
                    if let Some(i) = s.corners().iter().position(|q| q.dist(p) <= tol) {
                        s.drag = Some(ShapeDrag::Corner(i));
                        return;
                    }
                    if s.contains(p) {
                        s.drag = Some(ShapeDrag::Move { start: p, a0: s.a, b0: s.b });
                        return;
                    }
                }
                self.finish_session(true);
                let doc = &self.docs[self.cur];
                self.session = Session::Shape(Box::new(ShapeSession {
                    layer: doc.state.active,
                    base: doc.state.layer().px.clone(),
                    a: p,
                    b: p,
                    drag: Some(ShapeDrag::Create),
                    swap: button == 1,
                    last: Rect::EMPTY,
                }));
            }
            MOVE => {
                let mut changed = false;
                if let Session::Shape(s) = &mut self.session {
                    let constrain = |o: Pt, q: Pt| -> Pt {
                        if !shift {
                            return q;
                        }
                        let d = (q.x - o.x).abs().max((q.y - o.y).abs());
                        Pt::new(o.x + d * (q.x - o.x).signum(), o.y + d * (q.y - o.y).signum())
                    };
                    match &s.drag {
                        Some(ShapeDrag::Create) => {
                            s.b = constrain(s.a, p);
                            changed = true;
                        }
                        Some(ShapeDrag::Corner(i)) => {
                            match i {
                                0 => s.a = p,
                                1 => {
                                    s.b.x = p.x;
                                    s.a.y = p.y;
                                }
                                2 => s.b = p,
                                _ => {
                                    s.a.x = p.x;
                                    s.b.y = p.y;
                                }
                            }
                            changed = true;
                        }
                        Some(ShapeDrag::Move { start, a0, b0 }) => {
                            let (dx, dy) = (p.x - start.x, p.y - start.y);
                            s.a = Pt::new(a0.x + dx, a0.y + dy);
                            s.b = Pt::new(b0.x + dx, b0.y + dy);
                            changed = true;
                        }
                        None => {}
                    }
                }
                if changed {
                    self.refresh_session();
                }
            }
            UP | CANCEL => {
                let mut cancel = false;
                if let Session::Shape(s) = &mut self.session {
                    if matches!(s.drag, Some(ShapeDrag::Create)) && s.a.dist(s.b) < 1.0 {
                        cancel = true;
                    }
                    s.drag = None;
                }
                if cancel {
                    self.finish_session(false);
                }
                self.panels();
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------------------------------
    // Keyboard

    pub fn key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool, pressed: bool) -> bool {
        if text == " " && !matches!(self.session, Session::Text(_)) {
            self.space = pressed;
            self.update_cursor();
            return true;
        }
        if !pressed {
            return false;
        }
        let esc = key_str(Key::Escape);
        let enter = key_str(Key::Return);
        let bksp = key_str(Key::Backspace);
        let del = key_str(Key::Delete);
        let tab = key_str(Key::Tab);
        let backtab = key_str(Key::Backtab);

        // Text entry.
        if let Session::Text(_) = &self.session {
            if !ctrl && !alt {
                let mut handled = true;
                if text == esc {
                    self.finish_session(true);
                    return true;
                }
                if let Session::Text(t) = &mut self.session {
                    if text == enter {
                        t.text.push('\n');
                    } else if text == bksp {
                        t.text.pop();
                    } else if text.chars().all(|c| !c.is_control() && !('\u{F700}'..='\u{F8FF}').contains(&c)) && !text.is_empty() {
                        t.text.push_str(text);
                    } else {
                        handled = false;
                    }
                }
                if handled {
                    self.caret_on = true;
                    self.refresh_session();
                    self.redraw();
                    return true;
                }
            } else if ctrl && text == enter {
                self.finish_session(true);
                return true;
            }
        }

        if ctrl && (text == tab || text == backtab) {
            if !self.docs.is_empty() {
                let n = self.docs.len();
                let next = if text == backtab || shift { (self.cur + n - 1) % n } else { (self.cur + 1) % n };
                self.switch_doc(next);
            }
            return true;
        }
        if ctrl || alt {
            return false;
        }
        if text == esc {
            if self.session.is_editing() {
                self.finish_session(true);
            } else if matches!(self.session, Session::Move(_)) {
                self.session = Session::None;
                self.redraw();
            }
            return true;
        }
        if text == enter {
            self.finish_session(true);
            self.session = Session::None;
            self.redraw();
            return true;
        }
        if text == del {
            self.action("edit.erase");
            return true;
        }
        if text == bksp {
            self.action("edit.fill");
            return true;
        }
        let t = self.tool();
        let lower = text.to_lowercase();
        let next = match lower.as_str() {
            "s" => Some(match t {
                Tool::RectSelect => Tool::EllipseSelect,
                Tool::EllipseSelect => Tool::LassoSelect,
                Tool::LassoSelect => Tool::MagicWand,
                _ => Tool::RectSelect,
            }),
            "m" => Some(if t == Tool::MovePixels { Tool::MoveSelection } else { Tool::MovePixels }),
            "b" => Some(Tool::Brush),
            "p" => Some(Tool::Pencil),
            "e" => Some(Tool::Eraser),
            "f" => Some(Tool::Bucket),
            "g" => Some(Tool::Gradient),
            "k" => Some(Tool::Picker),
            "l" => Some(Tool::Clone),
            "r" => Some(Tool::Recolor),
            "t" => Some(Tool::Text),
            "o" => Some(if t == Tool::Line { Tool::Shape } else { Tool::Line }),
            "h" => Some(Tool::Pan),
            "z" => Some(Tool::Zoom),
            _ => None,
        };
        if let Some(n) = next {
            self.select_tool(n);
            return true;
        }
        match lower.as_str() {
            "x" => {
                self.action("color.swap");
                true
            }
            "d" => {
                self.action("color.reset");
                true
            }
            "[" | "]" => {
                let ui = self.ui();
                let g = ui.global::<App>();
                let w = g.get_brush_width();
                let step = (w / 10).max(1);
                let nw = if lower == "]" { w + step } else { w - step };
                g.set_brush_width(nw.clamp(1, 500));
                self.options_changed();
                true
            }
            _ => false,
        }
    }
}
