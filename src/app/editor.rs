//! The editor: owns documents and glues the Slint UI to the core engine.

use super::render::{self, Overlay, RenderParams};
use super::session::{Drag, Session};
use crate::core::document::Document;
use crate::core::geom::Pt;
use crate::core::surface::{Px, Surface};
use crate::core::text::FontLib;
use crate::{App, AppWindow, CurvePoint, DocTab, HistoryItem, LayerItem, ParamItem, Theme, Tool};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use web_time::Instant;

pub struct Models {
    pub layers: Rc<VecModel<LayerItem>>,
    pub history: Rc<VecModel<HistoryItem>>,
    pub tabs: Rc<VecModel<DocTab>>,
    pub params: Rc<VecModel<ParamItem>>,
    pub curve_points: Rc<VecModel<CurvePoint>>,
    pub recent: Rc<VecModel<slint::Color>>,
}

pub struct Editor {
    pub ui: slint::Weak<AppWindow>,
    pub docs: Vec<Document>,
    pub cur: usize,
    pub fonts: FontLib,
    pub session: Session,
    pub drag: Drag,
    /// Canvas size in logical pixels.
    pub canvas_l: (f32, f32),
    pub scale: f32,
    pub needs_render: bool,
    pub needs_panels: bool,
    pub hover: Option<Pt>,
    pub space: bool,
    pub phase: u32,
    pub last_phase: Instant,
    pub clone_src: Option<Pt>,
    pub clone_offset: Option<(i32, i32)>,
    pub last_stroke_end: Option<Pt>,
    pub fx: Option<super::actions::FxSession>,
    pub lp: Option<super::actions::LayerPropsSession>,
    pub last_fx: Option<(String, Vec<f32>)>,
    pub clipboard: Option<Surface>,
    pub after_save: Option<super::actions::AfterSave>,
    pub pending_close: Option<super::actions::AfterSave>,
    pub pending_save_path: Option<std::path::PathBuf>,
    pub picker_prev: Option<Tool>,
    pub recent: Vec<Px>,
    pub caret_on: bool,
    pub last_caret: Instant,
    pub models: Models,
    pub exiting: bool,
    pub system_scheme: slint::language::ColorScheme,
    pub recent_files: Vec<std::path::PathBuf>,
    pub glyphs: Option<render::Glyphs>,
    pub base_cache: Option<render::BaseCache>,
    #[allow(dead_code)]
    pub last_settings_save: Instant,
}

thread_local! {
    static EDITOR: RefCell<Option<Rc<RefCell<Editor>>>> = const { RefCell::new(None) };
}

/// Runs `f` with the editor, unless it is already borrowed (re-entrant call).
pub fn with_editor(f: impl FnOnce(&mut Editor)) {
    let ed = EDITOR.with(|e| e.borrow().clone());
    if let Some(ed) = ed {
        if let Ok(mut ed) = ed.try_borrow_mut() {
            f(&mut ed);
        }
    }
}

pub const PALETTE: [u32; 28] = [
    0x000000, 0x404040, 0x808080, 0xbfbfbf, 0xffffff, 0x7f1d1d, 0xef4444, 0xf97316, 0xf59e0b, 0xeab308, 0x84cc16, 0x22c55e, 0x10b981, 0x14b8a6,
    0x06b6d4, 0x0ea5e9, 0x3b82f6, 0x6366f1, 0x8b5cf6, 0xa855f7, 0xd946ef, 0xec4899, 0xf43f5e, 0x78350f, 0xa16207, 0x365314, 0x1e3a8a, 0x581c87,
];

pub fn color_to_px(c: slint::Color) -> Px {
    [c.red(), c.green(), c.blue(), c.alpha()]
}
pub fn px_to_color(p: Px) -> slint::Color {
    slint::Color::from_argb_u8(p[3], p[0], p[1], p[2])
}

pub fn run(files: Vec<std::path::PathBuf>) -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let g = ui.global::<App>();
    g.set_version(env!("CARGO_PKG_VERSION").into());

    let models = Models {
        layers: Rc::new(VecModel::default()),
        history: Rc::new(VecModel::default()),
        tabs: Rc::new(VecModel::default()),
        params: Rc::new(VecModel::default()),
        curve_points: Rc::new(VecModel::default()),
        recent: Rc::new(VecModel::default()),
    };
    g.set_layers(ModelRc::from(models.layers.clone()));
    g.set_history(ModelRc::from(models.history.clone()));
    g.set_tabs(ModelRc::from(models.tabs.clone()));
    g.set_fx_params(ModelRc::from(models.params.clone()));
    g.set_curve_points(ModelRc::from(models.curve_points.clone()));
    g.set_recent(ModelRc::from(models.recent.clone()));
    let palette: Vec<slint::Color> = PALETTE.iter().map(|c| slint::Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, *c as u8)).collect();
    g.set_palette(ModelRc::new(VecModel::from(palette)));
    let blend: Vec<SharedString> = crate::core::blend::BLEND_MODES.iter().map(|m| m.name().into()).collect();
    g.set_blend_modes(ModelRc::new(VecModel::from(blend)));
    let shapes: Vec<SharedString> = crate::core::paint::SHAPES.iter().map(|s| s.name().into()).collect();
    g.set_shape_names(ModelRc::new(VecModel::from(shapes)));

    let fonts = FontLib::new();
    let fam: Vec<SharedString> = fonts.families.iter().map(|f| f.as_str().into()).collect();
    g.set_fonts(ModelRc::new(VecModel::from(fam)));

    let ed = Rc::new(RefCell::new(Editor {
        ui: ui.as_weak(),
        docs: vec![],
        cur: 0,
        fonts,
        session: Session::None,
        drag: Drag::None,
        canvas_l: (800.0, 600.0),
        scale: 1.0,
        needs_render: true,
        needs_panels: true,
        hover: None,
        space: false,
        phase: 0,
        last_phase: Instant::now(),
        clone_src: None,
        clone_offset: None,
        last_stroke_end: None,
        fx: None,
        lp: None,
        last_fx: None,
        clipboard: None,
        after_save: None,
        pending_close: None,
        pending_save_path: None,
        picker_prev: None,
        recent: vec![],
        caret_on: true,
        last_caret: Instant::now(),
        models,
        exiting: false,
        system_scheme: ui.global::<crate::Palette>().get_color_scheme(),
        recent_files: vec![],
        glyphs: None,
        base_cache: None,
        last_settings_save: Instant::now(),
    }));
    EDITOR.with(|e| *e.borrow_mut() = Some(ed.clone()));

    g.on_canvas_pointer(|kind, button, x, y, ctrl, shift, alt| with_editor(|e| e.pointer(kind, button, x, y, ctrl, shift, alt)));
    g.on_canvas_scroll(|dx, dy, x, y, ctrl, shift| {
        let mut r = false;
        with_editor(|e| r = e.scroll(dx, dy, x, y, ctrl, shift));
        r
    });
    g.on_canvas_size(|w, h| with_editor(|e| e.canvas_resized(w, h)));
    g.on_key(|text, ctrl, shift, alt, pressed| {
        let mut r = false;
        with_editor(|e| r = e.key(&text, ctrl, shift, alt, pressed));
        r
    });
    g.on_action(|id| {
        let id = id.to_string();
        with_editor(move |e| e.action(&id))
    });
    g.on_tool_selected(|t| with_editor(|e| e.select_tool(t)));
    g.on_option_changed(|| with_editor(|e| e.options_changed()));
    g.on_color_changed(|| with_editor(|e| e.options_changed()));
    g.on_pick_palette(|c, right| {
        with_editor(|e| {
            let ui = e.ui();
            let g = ui.global::<App>();
            if right {
                g.set_secondary(c);
            } else {
                g.set_primary(c);
            }
            e.options_changed();
        })
    });
    g.on_layer_select(|i| with_editor(|e| e.layer_select_ui(i)));
    g.on_layer_toggle(|i| with_editor(|e| e.layer_toggle_ui(i)));
    g.on_layer_opacity(|v, commit| with_editor(|e| e.layer_opacity(v, commit)));
    g.on_layer_blend(|i| with_editor(|e| e.layer_blend(i)));
    g.on_history_goto(|i| with_editor(|e| e.history_goto(i)));
    g.on_tab_select(|i| with_editor(|e| e.switch_doc(i as usize)));
    g.on_tab_close(|i| with_editor(|e| e.request_close(i as usize)));
    g.on_zoom_set(|z| with_editor(|e| e.set_zoom_centered(z / 100.0)));
    g.on_fx_param(|i, v| with_editor(|e| e.fx_param(i as usize, v)));
    g.on_fx_ok(|| with_editor(|e| e.fx_ok()));
    g.on_fx_cancel(|| with_editor(|e| e.fx_cancel()));
    g.on_fx_reset(|| with_editor(|e| e.fx_reset()));
    g.on_new_image(|w, h, bg| with_editor(|e| e.new_image(w, h, bg)));
    g.on_resize_image(|w, h, m| with_editor(|e| e.resize_image(w, h, m)));
    g.on_canvas_resize(|w, h, a| with_editor(|e| e.canvas_resize(w, h, a)));
    g.on_layer_props(|name, vis, blend, op| {
        let name = name.to_string();
        with_editor(move |e| e.layer_props_live(name, vis, blend, op))
    });
    g.on_layer_props_ok(|| with_editor(|e| e.layer_props_ok()));
    g.on_layer_props_cancel(|| with_editor(|e| e.layer_props_cancel()));
    g.on_curve_pointer(|k, b, x, y| with_editor(|e| e.curve_pointer(k, b, x, y)));
    g.on_curve_channel_changed(|| with_editor(|e| e.curve_sync()));
    g.on_curve_reset(|| with_editor(|e| e.curve_reset()));
    g.on_confirm(|c| with_editor(|e| e.confirm(c)));
    g.on_jpeg_ok(|| with_editor(|e| e.jpeg_ok()));
    g.on_export_file(|name, fmt| {
        let name = name.to_string();
        with_editor(move |e| e.export_file(name, fmt))
    });
    g.set_is_web(cfg!(target_arch = "wasm32"));

    ui.window().on_close_requested(|| {
        let mut resp = slint::CloseRequestResponse::HideWindow;
        with_editor(|e| {
            if e.docs.iter().any(|d| d.is_modified()) {
                resp = slint::CloseRequestResponse::KeepWindowShown;
                if !e.exiting {
                    e.exiting = true;
                    e.continue_exit();
                }
            }
        });
        resp
    });

    // Frame timer: renders when needed and animates the selection outline.
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), || {
        with_editor(|e| e.tick());
    });

    {
        let mut e = ed.borrow_mut();
        e.load_settings();
        match std::env::var("NEOBRUSH_THEME").as_deref() {
            Ok("light") => e.set_theme(1),
            Ok("dark") => e.set_theme(2),
            _ => {}
        }
        for f in files {
            e.open_path(&f);
        }
        e.sync_all();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let r = ui.run();
        ed.borrow().save_settings();
        r
    }
    // On the web `run` returns immediately while the browser drives the event loop,
    // so the window and frame timer have to outlive this function.
    #[cfg(target_arch = "wasm32")]
    {
        fit_to_browser(&ui);
        let weak = ui.as_weak();
        let on_resize = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            if let Some(ui) = weak.upgrade() {
                fit_to_browser(&ui);
            }
        });
        if let Some(w) = web_sys::window() {
            use wasm_bindgen::JsCast;
            let _ = w.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
        }
        on_resize.forget();
        let r = ui.run();
        std::mem::forget(timer);
        std::mem::forget(ui);
        r
    }
}

/// Makes the Slint window fill the browser viewport.
#[cfg(target_arch = "wasm32")]
fn fit_to_browser(ui: &AppWindow) {
    let Some(w) = web_sys::window() else { return };
    let width = w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0);
    let height = w.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
    ui.window().set_size(slint::LogicalSize::new(width as f32, height as f32));
}

impl Editor {
    pub fn ui(&self) -> AppWindow {
        self.ui.upgrade().expect("ui alive")
    }

    pub fn doc(&self) -> Option<&Document> {
        self.docs.get(self.cur)
    }
    pub fn doc_mut(&mut self) -> Option<&mut Document> {
        self.docs.get_mut(self.cur)
    }

    pub fn tool(&self) -> Tool {
        self.ui().global::<App>().get_tool()
    }

    pub fn primary(&self) -> Px {
        color_to_px(self.ui().global::<App>().get_primary())
    }
    pub fn secondary(&self) -> Px {
        color_to_px(self.ui().global::<App>().get_secondary())
    }

    pub fn redraw(&mut self) {
        self.needs_render = true;
    }
    pub fn panels(&mut self) {
        self.needs_panels = true;
        self.needs_render = true;
    }

    pub fn tick(&mut self) {
        // The web build has no clean shutdown, so persist settings every few seconds.
        #[cfg(target_arch = "wasm32")]
        if self.last_settings_save.elapsed() > Duration::from_secs(3) {
            self.last_settings_save = Instant::now();
            self.save_settings();
        }
        // Marching ants & caret animation.
        let has_sel = self.doc().map(|d| d.state.selection.is_active()).unwrap_or(false);
        if has_sel && self.last_phase.elapsed() > Duration::from_millis(120) {
            self.phase = self.phase.wrapping_add(1);
            self.last_phase = Instant::now();
            self.needs_render = true;
        }
        if matches!(self.session, Session::Text(_)) && self.last_caret.elapsed() > Duration::from_millis(530) {
            self.caret_on = !self.caret_on;
            self.last_caret = Instant::now();
            self.needs_render = true;
        }
        self.fx_check_view();
        if self.needs_panels {
            self.needs_panels = false;
            if let Some(d) = self.docs.get_mut(self.cur) {
                d.update_composite();
            }
            self.sync_panels();
        }
        if self.needs_render {
            self.needs_render = false;
            self.render_canvas();
        }
    }

    pub fn canvas_resized(&mut self, w: f32, h: f32) {
        self.canvas_l = (w.max(1.0), h.max(1.0));
        self.scale = self.ui().window().scale_factor();
        for i in 0..self.docs.len() {
            if self.docs[i].view.needs_fit || self.docs[i].view.auto {
                self.fit_doc(i, true);
            }
        }
        self.redraw();
    }

    pub fn canvas_px(&self) -> (u32, u32) {
        ((self.canvas_l.0 * self.scale).round() as u32, (self.canvas_l.1 * self.scale).round() as u32)
    }

    pub fn render_canvas(&mut self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        self.scale = ui.window().scale_factor();
        let (cw, ch) = self.canvas_px();
        let dark = ui.global::<Theme>().get_dark();
        let grid = g.get_pixel_grid();
        let overlay = self.overlay();
        let phase = self.phase;
        let rulers = g.get_show_rulers();
        if rulers {
            let px = (10.0 * self.scale).round();
            if self.glyphs.as_ref().map(|gl| gl.px != px).unwrap_or(true) {
                self.glyphs = Some(render::Glyphs::new(px));
            }
        }
        let hover = self.hover;
        let scale = self.scale;
        let glyphs = if rulers { self.glyphs.as_ref() } else { None };
        let Some(doc) = self.docs.get_mut(self.cur) else {
            return;
        };
        if doc.view.needs_fit {
            return;
        }
        doc.update_composite();
        let key = render::BaseKey::new(doc, cw, ch, dark, grid);
        // Re-render the cached base only where something changed; overlays go on a copy.
        match &mut self.base_cache {
            Some(c) if c.key == key => {
                if !doc.view_dirty.is_empty() {
                    let region = key.doc_to_screen(doc.view_dirty);
                    render::render_base(doc, c.buf.make_mut_slice(), &key, region);
                }
            }
            slot => {
                let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(key.w, key.h);
                render::render_base(doc, buf.make_mut_slice(), &key, crate::core::geom::Rect::from_size(key.w, key.h));
                *slot = Some(render::BaseCache { key, buf });
            }
        }
        doc.view_dirty = crate::core::geom::Rect::EMPTY;
        let mut frame = self.base_cache.as_ref().unwrap().buf.clone();
        render::draw_overlays(doc, frame.make_mut_slice(), &key, &RenderParams { dark, grid, phase, rulers: glyphs, hover, scale }, &overlay);
        g.set_canvas(slint::Image::from_rgba8(frame));
        g.set_zoom(doc.view.zoom * 100.0);
        self.update_status();
    }

    pub fn overlay(&self) -> Overlay {
        let mut ov = Overlay::default();
        let Some(doc) = self.doc() else { return ov };
        self.session.overlay(&mut ov, doc, self.caret_on);
        self.drag.overlay(&mut ov);
        let tool = self.tool();
        if let Some(h) = self.hover {
            let ui = self.ui();
            let g = ui.global::<App>();
            match tool {
                Tool::Brush | Tool::Eraser | Tool::Clone | Tool::Recolor => {
                    ov.circle = Some((h, g.get_brush_width() as f32 / 2.0));
                }
                _ => {}
            }
        }
        if tool == Tool::Clone {
            if let Some(src) = self.clone_src {
                let cross = match (&self.drag, self.clone_offset, self.hover) {
                    (Drag::Stroke(s), _, _) => Some(Pt::new(s.last.x + s.clone_dx as f32, s.last.y + s.clone_dy as f32)),
                    (_, Some((dx, dy)), Some(h)) => Some(Pt::new(h.x + dx as f32, h.y + dy as f32)),
                    _ => Some(src),
                };
                ov.cross = cross;
            }
        }
        ov
    }

    pub fn update_status(&self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        let Some(doc) = self.doc() else {
            g.set_status_pos("".into());
            g.set_status_size("".into());
            g.set_status_sel("".into());
            return;
        };
        g.set_status_size(format!("{} × {}", doc.state.w, doc.state.h).into());
        match self.hover {
            Some(p) => g.set_status_pos(format!("{}, {}", p.x.floor() as i32, p.y.floor() as i32).into()),
            None => g.set_status_pos("—".into()),
        }
        match &doc.state.selection.mask {
            Some(m) => g.set_status_sel(format!("{} × {}", m.bounds.width(), m.bounds.height()).into()),
            None => g.set_status_sel("".into()),
        }
    }

    pub fn status_hint(&self, t: Tool) -> &'static str {
        match t {
            Tool::RectSelect => "Drag to select a rectangle. Click to deselect.",
            Tool::EllipseSelect => "Drag to select an ellipse. Hold Shift for a circle.",
            Tool::LassoSelect => "Drag to draw a freeform selection.",
            Tool::MagicWand => "Click to select a region of similar color.",
            Tool::MovePixels => "Drag the selection to move its pixels.",
            Tool::MoveSelection => "Drag to move the selection outline.",
            Tool::Zoom => "Click to zoom in, right-click to zoom out.",
            Tool::Pan => "Drag to pan the view.",
            Tool::Bucket => "Click to fill an area. Right-click uses the secondary color.",
            Tool::Gradient => "Drag to draw a gradient. Right-drag swaps the colors.",
            Tool::Brush => "Drag to paint. Shift+click draws a straight line.",
            Tool::Eraser => "Drag to erase to transparency.",
            Tool::Pencil => "Draw single pixel, aliased lines.",
            Tool::Picker => "Click to pick the primary color, right-click for secondary.",
            Tool::Clone => "Ctrl+click to set the source, then paint.",
            Tool::Recolor => "Replaces the secondary color with the primary color.",
            Tool::Text => "Click to place text, then type.",
            Tool::Line => "Drag to draw a line, then drag the handles to bend it.",
            Tool::Shape => "Drag to draw a shape, then adjust with the handles.",
        }
    }

    /// Updates layers, history, tabs and the misc UI properties.
    pub fn sync_panels(&mut self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_has_doc(!self.docs.is_empty());
        // Tabs
        let tabs: Vec<DocTab> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let comp = &d.composite;
                DocTab {
                    title: d.title.as_str().into(),
                    thumb: render::thumbnail(comp.w, comp.h, 64, |x, y| comp.get(x, y)),
                    modified: d.is_modified(),
                    active: i == self.cur,
                }
            })
            .collect();
        self.models.tabs.set_vec(tabs);
        let Some(doc) = self.docs.get(self.cur) else {
            self.models.layers.set_vec(vec![]);
            self.models.history.set_vec(vec![]);
            g.set_can_undo(false);
            g.set_can_redo(false);
            g.set_has_selection(false);
            g.set_window_title("Neobrush".into());
            return;
        };
        let st = &doc.state;
        let layers: Vec<LayerItem> = st
            .layers
            .iter()
            .enumerate()
            .rev()
            .map(|(i, l)| LayerItem {
                name: l.name.as_str().into(),
                visible: l.visible,
                opacity: (l.opacity * 100.0).round() as i32,
                blend: l.blend.name().into(),
                thumb: render::thumbnail(st.w, st.h, 76, |x, y| l.px.get(x, y)),
                selected: i == st.active,
            })
            .collect();
        self.models.layers.set_vec(layers);
        let h = &doc.history;
        let hist: Vec<HistoryItem> = h
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| HistoryItem { name: e.name.as_str().into(), icon: e.icon.into(), undone: i > h.index, current: i == h.index })
            .collect();
        if self.models.history.row_count() != hist.len() || hist.iter().enumerate().any(|(i, it)| self.models.history.row_data(i).as_ref() != Some(it)) {
            self.models.history.set_vec(hist);
        }
        g.set_history_index(h.index as i32);
        g.set_can_undo(h.can_undo());
        g.set_can_redo(h.can_redo());
        g.set_has_selection(st.selection.is_active());
        g.set_is_editing(self.session.is_editing());
        g.set_layer_count(st.layers.len() as i32);
        let l = st.layer();
        g.set_active_layer_blend(l.blend.name().into());
        g.set_active_layer_opacity((l.opacity * 100.0).round() as i32);
        g.set_image_w(st.w as i32);
        g.set_image_h(st.h as i32);
        let title = format!("{}{} — Neobrush", doc.title, if doc.is_modified() { " •" } else { "" });
        g.set_window_title(title.into());
        let rec: Vec<slint::Color> = self.recent.iter().map(|p| px_to_color(*p)).collect();
        self.models.recent.set_vec(rec);
        self.update_status();
    }

    pub fn sync_all(&mut self) {
        let t = self.tool();
        let hint = if self.docs.is_empty() { "" } else { self.status_hint(t) };
        self.ui().global::<App>().set_status_hint(hint.into());
        self.update_cursor();
        self.panels();
    }

    pub fn push_recent(&mut self, c: Px) {
        if c[3] == 0 {
            return;
        }
        self.recent.retain(|r| *r != c);
        self.recent.insert(0, c);
        self.recent.truncate(14);
    }

    // ------------------------------------------------------------------------------------------
    // View

    pub fn to_doc(&self, lx: f32, ly: f32) -> Pt {
        let Some(d) = self.doc() else { return Pt::default() };
        let sx = lx * self.scale;
        let sy = ly * self.scale;
        Pt::new((sx - d.view.ox) / d.view.zoom, (sy - d.view.oy) / d.view.zoom)
    }

    pub fn fit_doc(&mut self, i: usize, initial: bool) {
        let (cw, ch) = self.canvas_px();
        if cw < 50 || ch < 50 {
            return;
        }
        let d = &mut self.docs[i];
        let margin = 48.0 * self.scale;
        let mut z = ((cw as f32 - margin) / d.state.w as f32).min((ch as f32 - margin) / d.state.h as f32);
        if initial {
            z = z.min(1.0);
        }
        let z = z.max(0.01);
        d.view.zoom = z;
        d.view.ox = ((cw as f32 - d.state.w as f32 * z) / 2.0).round();
        d.view.oy = ((ch as f32 - d.state.h as f32 * z) / 2.0).round();
        d.view.needs_fit = false;
        d.view.auto = initial;
        self.redraw();
    }

    /// Part of the document currently visible in the viewport (with a small margin).
    pub fn visible_doc_rect(&self) -> crate::core::geom::Rect {
        use crate::core::geom::Rect;
        let Some(d) = self.doc() else { return Rect::EMPTY };
        let (cw, ch) = self.canvas_px();
        let v = d.view;
        let r = Rect::new(
            ((-v.ox) / v.zoom).floor() as i32 - 2,
            ((-v.oy) / v.zoom).floor() as i32 - 2,
            ((cw as f32 - v.ox) / v.zoom).ceil() as i32 + 2,
            ((ch as f32 - v.oy) / v.zoom).ceil() as i32 + 2,
        );
        r.intersect(&d.state.rect())
    }

    pub fn clamp_view(&mut self) {
        let (cw, ch) = self.canvas_px();
        let s = self.scale;
        let Some(d) = self.doc_mut() else { return };
        d.view.auto = false;
        let dw = d.state.w as f32 * d.view.zoom;
        let dh = d.state.h as f32 * d.view.zoom;
        let m = 64.0 * s;
        d.view.ox = d.view.ox.clamp(m - dw, cw as f32 - m);
        d.view.oy = d.view.oy.clamp(m - dh, ch as f32 - m);
    }

    /// Zooms keeping the screen point (physical px) fixed.
    pub fn zoom_at(&mut self, z: f32, sx: f32, sy: f32) {
        let Some(d) = self.doc_mut() else { return };
        let z = z.clamp(0.01, 64.0);
        let dx = (sx - d.view.ox) / d.view.zoom;
        let dy = (sy - d.view.oy) / d.view.zoom;
        d.view.zoom = z;
        d.view.ox = (sx - dx * z).round();
        d.view.oy = (sy - dy * z).round();
        self.clamp_view();
        self.redraw();
    }

    pub fn set_zoom_centered(&mut self, z: f32) {
        let (cw, ch) = self.canvas_px();
        self.zoom_at(z, cw as f32 / 2.0, ch as f32 / 2.0);
    }

    pub fn zoom_step(&mut self, dir: i32, at: Option<(f32, f32)>) {
        const STEPS: [f32; 27] = [
            0.01, 0.02, 0.03, 0.05, 0.08, 0.12, 0.17, 0.25, 0.33, 0.5, 0.66, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0,
        ];
        let Some(d) = self.doc() else { return };
        let z = d.view.zoom;
        let nz = if dir > 0 {
            STEPS.iter().copied().find(|s| *s > z * 1.001).unwrap_or(64.0)
        } else {
            STEPS.iter().rev().copied().find(|s| *s < z * 0.999).unwrap_or(0.01)
        };
        let (cw, ch) = self.canvas_px();
        let (sx, sy) = at.unwrap_or((cw as f32 / 2.0, ch as f32 / 2.0));
        self.zoom_at(nz, sx, sy);
    }

    pub fn scroll(&mut self, dx: f32, dy: f32, x: f32, y: f32, ctrl: bool, shift: bool) -> bool {
        if self.doc().is_none() {
            return false;
        }
        let s = self.scale;
        if ctrl {
            let z = self.doc().unwrap().view.zoom;
            let k = (dy * 0.0025).exp();
            self.zoom_at(z * k, x * s, y * s);
        } else {
            let (mut dx, mut dy) = (dx, dy);
            if shift && dx == 0.0 {
                std::mem::swap(&mut dx, &mut dy);
            }
            let d = self.doc_mut().unwrap();
            d.view.ox += dx * s;
            d.view.oy += dy * s;
            self.clamp_view();
            self.redraw();
        }
        true
    }

    pub fn update_cursor(&self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        let c = if matches!(self.drag, Drag::Pan { .. }) {
            4
        } else if self.space {
            3
        } else {
            match g.get_tool() {
                Tool::Pan => 3,
                Tool::MovePixels | Tool::MoveSelection => 2,
                Tool::Text => 5,
                Tool::Brush | Tool::Eraser | Tool::Clone | Tool::Recolor => {
                    if g.get_brush_width() as f32 * self.doc().map(|d| d.view.zoom).unwrap_or(1.0) >= 6.0 {
                        1
                    } else {
                        1
                    }
                }
                _ => 1,
            }
        };
        let c = self.session.cursor_override(self).unwrap_or(c);
        g.set_cursor(c);
    }

    pub fn options_changed(&mut self) {
        self.refresh_session();
        self.update_cursor();
        self.redraw();
    }

    pub fn message(&self, title: &str, text: &str) {
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_message_title(title.into());
        g.set_message_text(text.into());
        g.set_dialog("message".into());
    }

    /// 0 = follow system, 1 = light, 2 = dark
    pub fn set_theme(&mut self, mode: i32) {
        use slint::language::ColorScheme;
        let ui = self.ui();
        let p = ui.global::<crate::Palette>();
        p.set_color_scheme(match mode {
            1 => ColorScheme::Light,
            2 => ColorScheme::Dark,
            _ => self.system_scheme,
        });
        ui.global::<App>().set_theme_mode(mode);
        self.panels();
    }

    pub fn refocus(&self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_refocus(g.get_refocus() + 1);
    }
}
