//! Voxora görsel dili — konsol / yayın masası.
//! Renkler C# temasıyla birebir; tipografi Bahnschrift + Segoe + Cascadia.

use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals,
};
use std::sync::Arc;

pub const DISPLAY: &str = "display";
pub const MONO: &str = "mono";
pub const ICONS: &str = "icons";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub divider: Color32,
    pub input: Color32,
    pub input_border: Color32,
    pub ink: Color32,
    pub ink2: Color32,
    pub ink3: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_dim: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub deck: Color32,
    pub rail: Color32,
    pub glow: Color32,
}

pub fn palette(mode: ThemeMode) -> Palette {
    match mode {
        ThemeMode::Dark => Palette {
            bg: Color32::from_rgb(0x0E, 0x0C, 0x0A),
            panel: Color32::from_rgb(0x14, 0x11, 0x0E),
            surface: Color32::from_rgb(0x1A, 0x16, 0x12),
            surface_hi: Color32::from_rgb(0x24, 0x1F, 0x19),
            divider: Color32::from_rgb(0x2C, 0x25, 0x1E),
            input: Color32::from_rgb(0x16, 0x12, 0x0F),
            input_border: Color32::from_rgb(0x3A, 0x31, 0x28),
            ink: Color32::from_rgb(0xF4, 0xED, 0xE3),
            ink2: Color32::from_rgb(0xB4, 0xA7, 0x97),
            ink3: Color32::from_rgb(0x7F, 0x72, 0x63),
            accent: Color32::from_rgb(0xFF, 0x5C, 0x35),
            accent_hover: Color32::from_rgb(0xFF, 0x7D, 0x5C),
            accent_dim: Color32::from_rgb(0x3A, 0x21, 0x19),
            on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            danger: Color32::from_rgb(0xE2, 0x57, 0x4C),
            success: Color32::from_rgb(0x8F, 0xD4, 0x6B),
            warning: Color32::from_rgb(0xE5, 0xB4, 0x5C),
            deck: Color32::from_rgb(0x12, 0x10, 0x0D),
            rail: Color32::from_rgb(0x17, 0x13, 0x0F),
            glow: Color32::from_rgb(0x2A, 0x12, 0x09),
        },
        ThemeMode::Light => Palette {
            bg: Color32::from_rgb(0xF1, 0xEC, 0xE3),
            panel: Color32::from_rgb(0xF8, 0xF4, 0xED),
            surface: Color32::from_rgb(0xFC, 0xFA, 0xF5),
            surface_hi: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            divider: Color32::from_rgb(0xDF, 0xD6, 0xC8),
            input: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            input_border: Color32::from_rgb(0xD2, 0xC7, 0xB7),
            ink: Color32::from_rgb(0x19, 0x15, 0x12),
            ink2: Color32::from_rgb(0x57, 0x4C, 0x41),
            ink3: Color32::from_rgb(0x8B, 0x7D, 0x6C),
            accent: Color32::from_rgb(0xD8, 0x43, 0x1A),
            accent_hover: Color32::from_rgb(0xEE, 0x5C, 0x33),
            accent_dim: Color32::from_rgb(0xF8, 0xDD, 0xD3),
            on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            danger: Color32::from_rgb(0xB3, 0x38, 0x2E),
            success: Color32::from_rgb(0x3F, 0x7A, 0x34),
            warning: Color32::from_rgb(0x9A, 0x6B, 0x12),
            deck: Color32::from_rgb(0xF7, 0xF2, 0xEA),
            rail: Color32::from_rgb(0xF7, 0xF2, 0xEA),
            glow: Color32::from_rgb(0xFB, 0xE3, 0xDA),
        },
    }
}

pub fn font_display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY.into()))
}
pub fn font_mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MONO.into()))
}
pub fn font_icon(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICONS.into()))
}
pub fn font_body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let root = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("Fonts");

    for (key, file) in [
        ("bahnschrift", "bahnschrift.ttf"),
        ("cascadia", "CascadiaMono.ttf"),
        ("segoe", "SegUIVar.ttf"),
        ("segoeui", "segoeui.ttf"),
        ("seguisym", "seguisym.ttf"),
        ("segmdl2", "segmdl2.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(root.join(file)) {
            fonts
                .font_data
                .insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
        }
    }

    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let monospace = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let mut body: Vec<String> = Vec::new();
    if fonts.font_data.contains_key("segoe") {
        body.push("segoe".into());
    } else if fonts.font_data.contains_key("segoeui") {
        body.push("segoeui".into());
    }
    body.extend(proportional.iter().cloned());
    if fonts.font_data.contains_key("seguisym") {
        body.push("seguisym".into());
    }
    fonts.families.insert(FontFamily::Proportional, body.clone());

    let mut mono: Vec<String> = Vec::new();
    if fonts.font_data.contains_key("cascadia") {
        mono.push("cascadia".into());
    }
    mono.extend(monospace.iter().cloned());
    fonts.families.insert(FontFamily::Monospace, mono.clone());
    fonts
        .families
        .insert(FontFamily::Name(MONO.into()), mono);

    let display = if fonts.font_data.contains_key("bahnschrift") {
        vec!["bahnschrift".to_string(), body[0].clone()]
    } else {
        body.clone()
    };
    fonts
        .families
        .insert(FontFamily::Name(DISPLAY.into()), display);

    let icons = if fonts.font_data.contains_key("segmdl2") {
        vec!["segmdl2".to_string()]
    } else {
        body.clone()
    };
    fonts
        .families
        .insert(FontFamily::Name(ICONS.into()), icons);

    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let p = palette(mode);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.animation_time = 0.14;
        style.text_styles = [
            (TextStyle::Heading, font_display(22.0)),
            (TextStyle::Body, font_body(13.0)),
            (TextStyle::Button, font_body(12.5)),
            (TextStyle::Small, font_body(11.0)),
            (TextStyle::Monospace, font_mono(11.0)),
        ]
        .into();

        let mut v = if mode == ThemeMode::Dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        v.panel_fill = p.panel;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.input;
        v.faint_bg_color = p.surface_hi;
        v.window_stroke = Stroke::new(1.0, p.divider);
        v.selection.bg_fill = p.accent;
        v.selection.stroke = Stroke::new(1.0, p.on_accent);
        v.hyperlink_color = p.accent;
        v.override_text_color = Some(p.ink);

        v.widgets.noninteractive.bg_fill = p.surface;
        v.widgets.noninteractive.weak_bg_fill = p.surface;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.divider);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.ink2);
        v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

        v.widgets.inactive.bg_fill = p.surface;
        v.widgets.inactive.weak_bg_fill = p.surface;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.input_border);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.inactive.corner_radius = egui::CornerRadius::same(8);

        v.widgets.hovered.bg_fill = p.surface_hi;
        v.widgets.hovered.weak_bg_fill = p.surface_hi;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.input_border);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.hovered.corner_radius = egui::CornerRadius::same(8);

        v.widgets.active.bg_fill = p.accent_dim;
        v.widgets.active.weak_bg_fill = p.accent_dim;
        v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.active.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.active.corner_radius = egui::CornerRadius::same(8);

        v.widgets.open.bg_fill = p.surface;
        v.widgets.open.bg_stroke = Stroke::new(1.0, p.accent);
        v.widgets.open.fg_stroke = Stroke::new(1.0, p.ink);
        v.widgets.open.corner_radius = egui::CornerRadius::same(8);

        style.visuals = v;
    });
}

#[allow(dead_code)]
pub fn styled_button(ui: &mut egui::Ui, text: &str, primary: bool, p: &Palette) -> egui::Response {
    let padding = egui::vec2(14.0, 8.0);
    let font = font_body(12.5);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, p.ink);
    let desired = galley.size() + padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let hovered = response.hovered();
    let bg = if primary {
        if hovered {
            p.accent_hover
        } else {
            p.accent
        }
    } else if hovered {
        p.surface_hi
    } else {
        p.surface
    };
    let border = if primary {
        egui::Stroke::NONE
    } else {
        Stroke::new(1.0, p.input_border)
    };
    ui.painter()
        .rect(rect, 8.0, bg, border, egui::StrokeKind::Inside);
    let color = if primary { p.on_accent } else { p.ink };
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}



