use crate::theme::{self, Palette};
use egui::{
    Align2, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

/// Düğme türleri (C# Theme.Base.xaml stilleriyle aynı ölçüler).
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Soft,
    Danger,
    Plain,
    Ghost,
}

/// İkon (Segoe MDL2) + metin içeren düğme; WPF AccentButton/SoftButton eşleniği.
pub fn icon_text_button(
    ui: &mut Ui,
    glyph: char,
    label: &str,
    kind: ButtonKind,
    p: &Palette,
) -> Response {
    let padding = match kind {
        ButtonKind::Primary => vec2(15.0, 9.0),
        _ => vec2(14.0, 7.0),
    };
    let text_color = match kind {
        ButtonKind::Primary | ButtonKind::Danger => p.on_accent,
        ButtonKind::Soft => p.accent,
        ButtonKind::Ghost => p.ink2,
        ButtonKind::Plain => p.ink,
    };
    let icon_color = text_color;
    let text = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::font_body(12.5), text_color);
    let icon = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), theme::font_icon(12.0), icon_color);
    let gap = 7.0;
    let content = vec2(
        icon.size().x + gap + text.size().x,
        icon.size().y.max(text.size().y),
    );
    let desired = vec2(content.x + padding.x * 2.0, (content.y + padding.y * 2.0).max(34.0));
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    let bg = match kind {
        ButtonKind::Primary => {
            if response.hovered() {
                p.accent_hover
            } else {
                p.accent
            }
        }
        ButtonKind::Soft => {
            if response.hovered() {
                p.surface_hi
            } else {
                p.accent_dim
            }
        }
        ButtonKind::Danger => {
            if response.hovered() {
                p.danger.gamma_multiply(1.12)
            } else {
                p.danger
            }
        }
        ButtonKind::Ghost => {
            if response.hovered() {
                p.surface_hi
            } else {
                Color32::TRANSPARENT
            }
        }
        ButtonKind::Plain => {
            if response.hovered() {
                p.surface_hi
            } else {
                p.surface
            }
        }
    };
    let border = match kind {
        ButtonKind::Plain => Stroke::new(1.0, p.input_border),
        ButtonKind::Primary | ButtonKind::Danger | ButtonKind::Soft => Stroke::NONE,
        ButtonKind::Ghost => Stroke::NONE,
    };
    ui.painter().rect(rect, 8.0, bg, border, StrokeKind::Inside);

    let x = rect.center().x - content.x / 2.0;
    let icon_pos = pos2(x, rect.center().y - icon.size().y / 2.0);
    ui.painter().galley(icon_pos, icon.clone(), icon_color);
    let text_pos = pos2(
        x + icon.size().x + gap,
        rect.center().y - text.size().y / 2.0,
    );
    ui.painter().galley(text_pos, text, text_color);
    response
}

/// Yalnızca ikon içeren 34x34 düğme (IconButton).
pub fn icon_button(ui: &mut Ui, glyph: char, tooltip: &str, p: &Palette) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, p.surface_hi);
    }
    let color = if response.hovered() { p.ink } else { p.ink2 };
    let galley = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), theme::font_icon(15.0), color);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response.on_hover_text(tooltip)
}

#[allow(dead_code)]
pub fn accent_button(ui: &mut Ui, label: &str, p: &Palette) -> Response {
    theme::styled_button(ui, label, true, p)
}

#[allow(dead_code)]
pub fn soft_button(ui: &mut Ui, label: &str, p: &Palette) -> Response {
    let padding = vec2(12.0, 6.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::font_body(12.0), p.accent);
    let desired = galley.size() + padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    let bg = if response.hovered() {
        p.surface_hi
    } else {
        p.accent_dim
    };
    ui.painter().rect_filled(rect, 8.0, bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, p.accent);
    response
}

pub fn ghost_button(ui: &mut Ui, label: &str, p: &Palette, danger: bool) -> Response {
    let color = if danger { p.danger } else { p.ink2 };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::font_body(12.0), color);
    let desired = galley.size() + vec2(14.0, 8.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, p.surface_hi);
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

pub fn icon_glyph(ui: &mut Ui, glyph: char, size: f32, color: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), theme::font_icon(size), color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(2.0, 2.0), Sense::hover());
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
}

/// Yerleşimi etkilemeden, verilen merkeze ikon çizer.
pub fn icon_glyph_at(ui: &Ui, center: egui::Pos2, glyph: char, size: f32, color: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), theme::font_icon(size), color);
    ui.painter()
        .galley(center - galley.size() / 2.0, galley, color);
}

/// Yerleşimi etkilemeden ikon + metin çizer (düğmelerin içi için).
pub fn paint_icon_text(
    ui: &Ui,
    center: egui::Pos2,
    glyph: char,
    label: &str,
    icon_size: f32,
    text_size: f32,
    gap: f32,
    icon_color: Color32,
    text_color: Color32,
) {
    let icon = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), theme::font_icon(icon_size), icon_color);
    let text = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::font_body(text_size),
        text_color,
    );
    let total = icon.size().x + gap + text.size().x;
    let x = center.x - total / 2.0;
    ui.painter().galley(
        pos2(x, center.y - icon.size().y / 2.0),
        icon.clone(),
        icon_color,
    );
    ui.painter().galley(
        pos2(x + icon.size().x + gap, center.y - text.size().y / 2.0),
        text,
        text_color,
    );
}

pub fn chip(ui: &mut Ui, text: &str, color: Color32, p: &Palette) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::font_mono(9.5), color);
    let desired = galley.size() + vec2(14.0, 4.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter().rect_filled(rect, 4.0, p.accent_dim);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

pub fn section(ui: &mut Ui, text: &str, p: &Palette) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::font_display(10.5))
            .color(p.ink2),
    );
}

/// Yatay fader (WPF Slider özel şablonu). Değiştiyse true.
pub fn fader(ui: &mut Ui, value: &mut f32, max: f32, width: f32, p: &Palette) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 6.0));
    ui.painter().rect_filled(track, 3.0, p.input);
    let fraction = (*value / max).clamp(0.0, 1.0);
    let fill = Rect::from_min_size(track.min, vec2(track.width() * fraction, 6.0));
    ui.painter().rect_filled(fill, 3.0, p.accent);
    let cap_x = track.left() + track.width() * fraction;
    let cap = Rect::from_center_size(pos2(cap_x, rect.center().y), vec2(11.0, 20.0));
    ui.painter()
        .rect(cap, 3.0, p.ink, Stroke::new(1.0, p.bg), StrokeKind::Inside);
    ui.painter().rect_filled(
        Rect::from_center_size(cap.center(), vec2(2.0, 9.0)),
        1.0,
        p.bg.gamma_multiply(0.6),
    );

    let mut changed = false;
    if response.dragged() || response.clicked() {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            let new = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) * max;
            if (new - *value).abs() > 0.0001 {
                *value = new;
                changed = true;
            }
        }
    }
    changed
}

/// Anahtar (WPF Switch). Değiştiyse true.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str, p: &Palette) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(response.id, *on, 0.16);
    let track = if *on { p.accent } else { p.input };
    ui.painter().rect_filled(rect, 11.0, track);
    if !*on {
        ui.painter().rect_stroke(
            rect,
            11.0,
            Stroke::new(1.0, p.input_border),
            StrokeKind::Inside,
        );
    }
    let x = rect.left() + 3.0 + (rect.width() - 20.0) * t;
    let thumb = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(16.0, 16.0));
    ui.painter().circle_filled(
        thumb.center(),
        8.0,
        if *on { p.on_accent } else { p.ink3 },
    );
    if !label.is_empty() {
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(label)
                .font(theme::font_body(11.5))
                .color(p.ink2),
        );
    }
    if response.clicked() {
        *on = !*on;
        return true;
    }
    false
}

pub fn meter(ui: &mut Ui, value: f32, width: f32, color: Color32, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 3.0, p.input);
    let fraction = value.clamp(0.0, 1.0);
    if fraction > 0.001 {
        let fill = Rect::from_min_size(rect.min, vec2(rect.width() * fraction, rect.height()));
        ui.painter().rect_filled(fill, 3.0, color);
    }
}

/// Daire dilimi (iki yay arasında).
#[allow(dead_code)]
pub fn wedge(
    painter: &egui::Painter,
    center: egui::Pos2,
    inner: f32,
    outer: f32,
    start_deg: f32,
    end_deg: f32,
    fill: Color32,
    stroke: Stroke,
) {
    let mut points = Vec::new();
    let steps = 10;
    for i in 0..=steps {
        let a = (start_deg + (end_deg - start_deg) * i as f32 / steps as f32).to_radians();
        points.push(pos2(center.x + a.cos() * outer, center.y + a.sin() * outer));
    }
    for i in (0..=steps).rev() {
        let a = (start_deg + (end_deg - start_deg) * i as f32 / steps as f32).to_radians();
        points.push(pos2(center.x + a.cos() * inner, center.y + a.sin() * inner));
    }
    painter.add(egui::Shape::convex_polygon(points, fill, stroke));
}

pub fn toast(ui: &mut Ui, text: &str, kind: ToastKind, p: &Palette, width: f32, age: f32) {
    let alpha = (1.0 - (age / 3.2).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let color = match kind {
        ToastKind::Info => p.accent,
        ToastKind::Warning => p.warning,
        ToastKind::Error => p.danger,
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, 34.0), Sense::hover());
    let bg = p.surface.gamma_multiply(0.96);
    ui.painter()
        .rect(rect, 8.0, bg, Stroke::new(1.0, color), StrokeKind::Inside);
    ui.painter().circle_filled(
        pos2(rect.left() + 16.0, rect.center().y),
        3.5,
        color.gamma_multiply(alpha),
    );
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        theme::font_body(12.0),
        p.ink.gamma_multiply(alpha),
    );
    ui.painter().galley(
        pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0),
        galley,
        p.ink,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Warning,
    Error,
}

pub fn combo(
    ui: &mut Ui,
    id: &str,
    selected: &mut Option<String>,
    items: &[String],
    width: f32,
    p: &Palette,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(selected.clone().unwrap_or_else(|| "—".into()))
        .show_ui(ui, |ui| {
            for item in items {
                let is = selected.as_deref() == Some(item.as_str());
                if ui.selectable_label(is, item).clicked() {
                    *selected = Some(item.clone());
                    changed = true;
                }
            }
        });
    let _ = p;
    changed
}

/// Düz metin etiketi (yardımcı).
pub fn text_at(painter: &egui::Painter, pos: egui::Pos2, text: &str, size: f32, color: Color32, align: Align2) {
    painter.text(pos, align, text, theme::font_body(size), color);
}

pub fn mono_at(painter: &egui::Painter, pos: egui::Pos2, text: &str, size: f32, color: Color32, align: Align2) {
    painter.text(pos, align, text, theme::font_mono(size), color);
}

pub fn display_at(painter: &egui::Painter, pos: egui::Pos2, text: &str, size: f32, color: Color32, align: Align2) {
    painter.text(pos, align, text, theme::font_display(size), color);
}

#[allow(dead_code)]
pub fn icon_size(size: f32) -> FontId {
    theme::font_icon(size)
}

/// Dikey ayırıcı (araç çubuğu grupları arasında).
pub fn divider_v(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, 22.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.divider);
}

/// Seçenek satırı (dil/tema seçimi): 44 px, seçiliyken vurgu çerçevesi + onay ikonu.
pub fn option_row(ui: &mut Ui, label: &str, selected: bool, leading: Option<Color32>, p: &Palette) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 44.0), Sense::click());
    let border = if selected { p.accent } else { p.divider };
    ui.painter().rect(
        rect,
        8.0,
        p.input,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let mut text_x = rect.left() + 16.0;
    if let Some(color) = leading {
        let sw = Rect::from_center_size(
            pos2(rect.left() + 26.0, rect.center().y),
            vec2(18.0, 18.0),
        );
        ui.painter().rect(sw, 4.0, color, Stroke::new(1.0, p.divider), StrokeKind::Inside);
        text_x = rect.left() + 46.0;
    }
    if selected {
        let galley = ui
            .painter()
            .layout_no_wrap("\u{E73E}".to_string(), theme::font_icon(12.0), p.accent);
        ui.painter().galley(
            pos2(rect.left() + 16.0, rect.center().y - galley.size().y / 2.0),
            galley,
            p.accent,
        );
        text_x = rect.left() + 38.0;
    }
    ui.painter().text(
        pos2(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::font_body(12.5),
        p.ink,
    );
    response
}
