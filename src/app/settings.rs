//! Persistent user preferences stored as a simple `key=value` file.

use super::editor::{color_to_px, px_to_color, Editor};
use crate::App;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::PathBuf;

fn hex(p: [u8; 4]) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", p[0], p[1], p[2], p[3])
}

fn parse_hex(s: &str) -> Option<[u8; 4]> {
    if s.len() != 8 {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?, b(6)?])
}

impl Editor {
    pub fn load_settings(&mut self) {
        let Some(text) = super::platform::read_settings() else { return };
        let ui = self.ui();
        let g = ui.global::<App>();
        let mut theme = 0;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let int = v.parse::<i32>().ok();
            let b = v == "1" || v == "true";
            match k {
                "theme" => theme = int.unwrap_or(0),
                "pixel_grid" => g.set_pixel_grid(b),
                "rulers" => g.set_show_rulers(b),
                "show_menubar" => g.set_show_menubar(b),
                "show_tools" => g.set_show_tools(b),
                "show_colors" => g.set_show_colors(b),
                "show_layers" => g.set_show_layers(b),
                "show_history" => g.set_show_history(b),
                "brush_width" => g.set_brush_width(int.unwrap_or(8).clamp(1, 500)),
                "hardness" => g.set_hardness(int.unwrap_or(75).clamp(0, 100)),
                "antialias" => g.set_antialias(b),
                "tolerance" => g.set_tolerance(int.unwrap_or(50).clamp(0, 100)),
                "font_family" => g.set_font_family(v.into()),
                "font_size" => g.set_font_size(int.unwrap_or(32).clamp(4, 400)),
                "jpeg_quality" => g.set_jpeg_quality(int.unwrap_or(92).clamp(1, 100)),
                "primary" => {
                    if let Some(c) = parse_hex(v) {
                        g.set_primary(px_to_color(c));
                    }
                }
                "secondary" => {
                    if let Some(c) = parse_hex(v) {
                        g.set_secondary(px_to_color(c));
                    }
                }
                "recent_file" => {
                    if !v.is_empty() && self.recent_files.len() < 10 {
                        self.recent_files.push(PathBuf::from(v));
                    }
                }
                "recent_cmd" => {
                    if !v.is_empty() && self.recent_cmds.len() < 30 {
                        self.recent_cmds.push(v.to_string());
                    }
                }
                "recent_color" => {
                    if let Some(c) = parse_hex(v) {
                        self.recent.push(c);
                    }
                }
                _ => {}
            }
        }
        if theme != 0 {
            self.set_theme(theme);
        }
        self.sync_recent_files();
    }

    pub fn save_settings(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let g = ui.global::<App>();
        let b = |v: bool| if v { "1" } else { "0" };
        let mut s = String::new();
        s += &format!("theme={}\n", g.get_theme_mode());
        s += &format!("pixel_grid={}\n", b(g.get_pixel_grid()));
        s += &format!("rulers={}\n", b(g.get_show_rulers()));
        s += &format!("show_menubar={}\n", b(g.get_show_menubar()));
        s += &format!("show_tools={}\n", b(g.get_show_tools()));
        s += &format!("show_colors={}\n", b(g.get_show_colors()));
        s += &format!("show_layers={}\n", b(g.get_show_layers()));
        s += &format!("show_history={}\n", b(g.get_show_history()));
        s += &format!("brush_width={}\n", g.get_brush_width());
        s += &format!("hardness={}\n", g.get_hardness());
        s += &format!("antialias={}\n", b(g.get_antialias()));
        s += &format!("tolerance={}\n", g.get_tolerance());
        s += &format!("font_family={}\n", g.get_font_family());
        s += &format!("font_size={}\n", g.get_font_size());
        s += &format!("jpeg_quality={}\n", g.get_jpeg_quality());
        s += &format!("primary={}\n", hex(color_to_px(g.get_primary())));
        s += &format!("secondary={}\n", hex(color_to_px(g.get_secondary())));
        for f in &self.recent_files {
            s += &format!("recent_file={}\n", f.display());
        }
        for c in &self.recent_cmds {
            s += &format!("recent_cmd={c}\n");
        }
        for c in &self.recent {
            s += &format!("recent_color={}\n", hex(*c));
        }
        super::platform::write_settings(&s);
    }

    pub fn add_recent_file(&mut self, p: &std::path::Path) {
        let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        self.recent_files.retain(|r| r != &p);
        self.recent_files.insert(0, p);
        self.recent_files.truncate(10);
        self.sync_recent_files();
    }

    pub fn sync_recent_files(&self) {
        let items: Vec<SharedString> = self.recent_files.iter().map(|p| p.display().to_string().into()).collect();
        self.ui().global::<App>().set_recent_files(ModelRc::new(VecModel::from(items)));
    }
}
