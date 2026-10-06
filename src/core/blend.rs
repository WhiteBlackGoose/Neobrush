//! Layer blend modes (Paint.NET set).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    ColorBurn,
    ColorDodge,
    Reflect,
    Glow,
    Overlay,
    Difference,
    Negation,
    Lighten,
    Darken,
    Screen,
    Xor,
    HardLight,
    SoftLight,
    Additive,
}

pub const BLEND_MODES: [BlendMode; 16] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::ColorBurn,
    BlendMode::ColorDodge,
    BlendMode::Reflect,
    BlendMode::Glow,
    BlendMode::Overlay,
    BlendMode::Difference,
    BlendMode::Negation,
    BlendMode::Lighten,
    BlendMode::Darken,
    BlendMode::Screen,
    BlendMode::Xor,
    BlendMode::HardLight,
    BlendMode::SoftLight,
    BlendMode::Additive,
];

impl BlendMode {
    pub fn name(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::ColorBurn => "Color Burn",
            BlendMode::ColorDodge => "Color Dodge",
            BlendMode::Reflect => "Reflect",
            BlendMode::Glow => "Glow",
            BlendMode::Overlay => "Overlay",
            BlendMode::Difference => "Difference",
            BlendMode::Negation => "Negation",
            BlendMode::Lighten => "Lighten",
            BlendMode::Darken => "Darken",
            BlendMode::Screen => "Screen",
            BlendMode::Xor => "Xor",
            BlendMode::HardLight => "Hard Light",
            BlendMode::SoftLight => "Soft Light",
            BlendMode::Additive => "Additive",
        }
    }
    pub fn index(self) -> usize {
        BLEND_MODES.iter().position(|m| *m == self).unwrap_or(0)
    }
    pub fn from_index(i: usize) -> BlendMode {
        BLEND_MODES.get(i).copied().unwrap_or_default()
    }
    /// OpenRaster composite-op name.
    pub fn ora_name(self) -> &'static str {
        match self {
            BlendMode::Normal => "svg:src-over",
            BlendMode::Multiply => "svg:multiply",
            BlendMode::ColorBurn => "svg:color-burn",
            BlendMode::ColorDodge => "svg:color-dodge",
            BlendMode::Overlay => "svg:overlay",
            BlendMode::Difference => "svg:difference",
            BlendMode::Lighten => "svg:lighten",
            BlendMode::Darken => "svg:darken",
            BlendMode::Screen => "svg:screen",
            BlendMode::HardLight => "svg:hard-light",
            BlendMode::SoftLight => "svg:soft-light",
            BlendMode::Additive => "svg:plus",
            BlendMode::Reflect => "neobrush:reflect",
            BlendMode::Glow => "neobrush:glow",
            BlendMode::Negation => "neobrush:negation",
            BlendMode::Xor => "neobrush:xor",
        }
    }
    pub fn from_ora_name(s: &str) -> BlendMode {
        BLEND_MODES.iter().copied().find(|m| m.ora_name() == s).unwrap_or_default()
    }

    /// Separable blend function on 0..1 channel values (b = backdrop, s = source).
    #[inline]
    fn f(self, b: f32, s: f32) -> f32 {
        match self {
            BlendMode::Normal => s,
            BlendMode::Multiply => b * s,
            BlendMode::ColorBurn => {
                if b >= 1.0 {
                    1.0
                } else if s <= 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - b) / s).min(1.0)
                }
            }
            BlendMode::ColorDodge => {
                if b <= 0.0 {
                    0.0
                } else if s >= 1.0 {
                    1.0
                } else {
                    (b / (1.0 - s)).min(1.0)
                }
            }
            BlendMode::Reflect => {
                if s >= 1.0 {
                    1.0
                } else {
                    (b * b / (1.0 - s)).min(1.0)
                }
            }
            BlendMode::Glow => {
                if b >= 1.0 {
                    1.0
                } else {
                    (s * s / (1.0 - b)).min(1.0)
                }
            }
            BlendMode::Overlay => BlendMode::HardLight.f(s, b),
            BlendMode::Difference => (b - s).abs(),
            BlendMode::Negation => 1.0 - (1.0 - b - s).abs(),
            BlendMode::Lighten => b.max(s),
            BlendMode::Darken => b.min(s),
            BlendMode::Screen => b + s - b * s,
            BlendMode::Xor => (((b * 255.0) as u8) ^ ((s * 255.0) as u8)) as f32 / 255.0,
            BlendMode::HardLight => {
                if s <= 0.5 {
                    b * 2.0 * s
                } else {
                    let s2 = 2.0 * s - 1.0;
                    b + s2 - b * s2
                }
            }
            BlendMode::SoftLight => {
                if s <= 0.5 {
                    b - (1.0 - 2.0 * s) * b * (1.0 - b)
                } else {
                    let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() };
                    b + (2.0 * s - 1.0) * (d - b)
                }
            }
            BlendMode::Additive => (b + s).min(1.0),
        }
    }

    /// Composites straight-alpha float source (0..1, alpha already including opacity)
    /// onto a premultiplied float backdrop (0..1).
    #[inline]
    pub fn composite(self, dst: &mut [f32; 4], s: [f32; 4]) {
        let sa = s[3];
        if sa <= 0.0 {
            return;
        }
        let da = dst[3];
        if self == BlendMode::Normal || da <= 0.0 {
            for i in 0..3 {
                dst[i] = s[i] * sa + dst[i] * (1.0 - sa);
            }
            dst[3] = sa + da * (1.0 - sa);
            return;
        }
        for i in 0..3 {
            let b = dst[i] / da;
            let blended = self.f(b, s[i]);
            // co = as*(1-ab)*Cs + as*ab*B + (1-as)*cb_premul
            dst[i] = sa * (1.0 - da) * s[i] + sa * da * blended + (1.0 - sa) * dst[i];
        }
        dst[3] = sa + da * (1.0 - sa);
    }
}
