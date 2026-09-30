//! Colours, fonts and Thai text fix-ups. Colours follow docs/DESIGN.md.

use eframe::egui::{self, Color32, FontFamily, FontId, TextureHandle, TextureOptions};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x0A, 0x0A, 0x0C);
pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x14, 0x17);
pub const SURFACE2: Color32 = Color32::from_rgb(0x1D, 0x1D, 0x22);
pub const OUTLINE: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x31);
pub const RED: Color32 = Color32::from_rgb(0xE6, 0x26, 0x39);
pub const RED_HI: Color32 = Color32::from_rgb(0xFF, 0x5A, 0x6A);
pub const RED_DEEP: Color32 = Color32::from_rgb(0x8C, 0x13, 0x20);
pub const TEXT: Color32 = Color32::from_rgb(0xF4, 0xF4, 0xF6);
pub const TEXT2: Color32 = Color32::from_rgb(0xA3, 0xA3, 0xAD);
pub const TEXT3: Color32 = Color32::from_rgb(0x6E, 0x6E, 0x78);
pub const GREEN: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x84);
pub const AMBER: Color32 = Color32::from_rgb(0xFF, 0xB0, 0x20);
pub const ERROR: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Gradients drawn by stretching tiny textures over rounded shapes.
pub struct Gradients {
    /// red → redHi, left to right.
    pub horizontal: TextureHandle,
}

impl Gradients {
    /// UV rect that spans exactly from the first texel's centre to the last.
    pub const UV_H: egui::Rect = egui::Rect { min: egui::pos2(0.25, 0.5), max: egui::pos2(0.75, 0.5) };

    fn new(ctx: &egui::Context) -> Gradients {
        let img = egui::ColorImage::new([2, 1], vec![RED, RED_HI]);
        Gradients { horizontal: ctx.load_texture("grad-h", img, TextureOptions::LINEAR) }
    }
}

pub fn setup(ctx: &egui::Context) -> Gradients {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("thai".to_owned(), Arc::new(egui::FontData::from_static(include_bytes!("../../assets/Loma.ttf"))));
    // First in line: Loma covers Latin too, and its Thai mark glyphs must win
    // over the icon font, which also uses the private-use area.
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&fam) {
            list.insert(0, "thai".to_owned());
        }
    }
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = SURFACE2;
    v.faint_bg_color = SURFACE2;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = RED;
    v.selection.stroke.color = TEXT;
    v.hyperlink_color = RED_HI;
    v.window_corner_radius = 14.into();
    v.menu_corner_radius = 12.into();
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(160) };
    for w in [&mut v.widgets.inactive, &mut v.widgets.noninteractive] {
        w.corner_radius = 10.into();
    }
    v.widgets.noninteractive.bg_stroke.color = OUTLINE;
    v.widgets.inactive.bg_fill = SURFACE2;
    v.widgets.inactive.weak_bg_fill = SURFACE2;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x26, 0x26, 0x2D);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x26, 0x26, 0x2D);
    v.widgets.hovered.bg_stroke.color = RED;
    v.widgets.hovered.corner_radius = 10.into();
    v.widgets.active.bg_fill = RED_DEEP;
    v.widgets.active.weak_bg_fill = RED_DEEP;
    v.widgets.active.corner_radius = 10.into();
    v.widgets.open.corner_radius = 10.into();
    v.text_cursor.stroke.color = RED_HI;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 7.0);
        s.spacing.interact_size.y = 30.0;
        s.text_styles.insert(egui::TextStyle::Body, font(15.0));
        s.text_styles.insert(egui::TextStyle::Button, font(15.0));
        s.text_styles.insert(egui::TextStyle::Small, font(12.5));
        s.text_styles.insert(egui::TextStyle::Heading, font(22.0));
    });
    Gradients::new(ctx)
}

/// The app icon (the same picture as the phone apps), also built into the
/// Windows .exe from assets/icon.ico.
pub const ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");

pub fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(ICON_PNG).unwrap_or_default()
}

/// egui draws text without OpenType shaping, so Thai tone marks would sit on
/// top of upper vowels. Swap in the pre-positioned mark glyphs (the classic
/// Windows private-use variants, which Loma carries) to place them correctly.
pub fn t(s: impl AsRef<str>) -> String {
    fn is_upper_vowel(c: char) -> bool {
        matches!(c, '\u{0E31}' | '\u{0E34}'..='\u{0E37}' | '\u{0E47}' | '\u{0E4D}')
    }
    fn is_tall(c: char) -> bool {
        matches!(c, '\u{0E1B}' | '\u{0E1D}' | '\u{0E1F}' | '\u{0E2C}')
    }
    fn is_tone(c: char) -> bool {
        ('\u{0E48}'..='\u{0E4C}').contains(&c)
    }
    let chars: Vec<char> = s.as_ref().chars().collect();
    let mut out = String::with_capacity(s.as_ref().len() + 8);
    let mut base = '\0';
    let mut had_upper = false;
    for (i, &c) in chars.iter().enumerate() {
        let shifted = |pua_base: u32, first: char| char::from_u32(pua_base + (c as u32 - first as u32)).unwrap_or(c);
        let r = if is_upper_vowel(c) {
            had_upper = true;
            if is_tall(base) {
                match c {
                    '\u{0E31}' => '\u{F710}',
                    '\u{0E47}' => '\u{F712}',
                    '\u{0E4D}' => '\u{F711}',
                    _ => shifted(0xF701, '\u{0E34}'),
                }
            } else {
                c
            }
        } else if is_tone(c) {
            let before_am = chars.get(i + 1) == Some(&'\u{0E33}');
            match (is_tall(base), had_upper || before_am) {
                (true, true) => shifted(0xF713, '\u{0E48}'),
                (true, false) => shifted(0xF705, '\u{0E48}'),
                (false, true) => c,
                (false, false) => shifted(0xF70A, '\u{0E48}'),
            }
        } else {
            if !('\u{0E30}'..='\u{0E3A}').contains(&c) && !('\u{0E47}'..='\u{0E4E}').contains(&c) {
                base = c;
                had_upper = false;
            }
            c
        };
        out.push(r);
    }
    out
}
