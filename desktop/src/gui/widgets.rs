//! Custom widgets: cards, switches, segmented controls, device rows, the
//! visualizer and the volume tube. Look and behaviour follow docs/DESIGN.md.

use super::theme::*;
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Ui,
};
use ssnd_core::ScopeSnapshot;
use std::time::Instant;

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

fn radius(r: f32) -> CornerRadius {
    CornerRadius::same(r.clamp(0.0, 255.0) as u8)
}

/// A rounded rect filled with one of the gradient textures.
fn gradient_rect(ui: &Ui, rect: Rect, r: f32, tex: &egui::TextureHandle, uv: Rect, tint: Color32) {
    ui.painter()
        .add(egui::epaint::RectShape::filled(rect, radius(r), tint).with_texture(tex.id(), uv));
}

pub fn card<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let r = egui::Frame::new()
        .fill(SURFACE)
        .corner_radius(16)
        .stroke(Stroke::new(1.0, OUTLINE))
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
        .inner;
    ui.add_space(12.0);
    r
}

pub fn section_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(t(text)).size(16.0).color(TEXT));
}

pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(t(text)).size(12.5).color(TEXT2));
}

/// Status pill with an optional (pulsing) dot.
pub fn pill(ui: &mut Ui, text: &str, color: Color32, dot: bool, pulse: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(t(text), font(13.0), color);
    let pad = vec2(11.0, 5.0);
    let dot_w = if dot { 14.0 } else { 0.0 };
    let size = vec2(galley.size().x + pad.x * 2.0 + dot_w, galley.size().y + pad.y * 2.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, radius(rect.height() / 2.0), color.gamma_multiply(0.14));
    if dot {
        let mut a = 1.0;
        if pulse {
            let time = ui.input(|i| i.time) as f32;
            a = 0.45 + 0.55 * (0.5 + 0.5 * (time * 4.0).sin());
            ui.ctx().request_repaint();
        }
        p.circle_filled(pos2(rect.left() + pad.x + 4.0, rect.center().y), 4.0, color.gamma_multiply(a));
    }
    p.galley(pos2(rect.left() + pad.x + dot_w, rect.top() + pad.y), galley, color);
    resp
}

/// iOS-style on/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(46.0, 26.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let k = ui.ctx().animate_bool_with_time(resp.id, *on, 0.14);
    let p = ui.painter();
    p.rect(rect, radius(13.0), mix(SURFACE2, RED, k), Stroke::new(1.0, mix(OUTLINE, RED, k)), StrokeKind::Inside);
    let x = egui::lerp(rect.left() + 13.0..=rect.right() - 13.0, k);
    p.circle_filled(pos2(x, rect.center().y + 1.0), 10.0, Color32::from_black_alpha(60));
    p.circle_filled(pos2(x, rect.center().y), 10.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A label on the left, a switch on the right.
pub fn toggle_row(ui: &mut Ui, label: &str, detail: Option<&str>, on: &mut bool) -> Response {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 56.0);
            ui.label(RichText::new(t(label)).size(15.0));
            if let Some(d) = detail {
                ui.label(RichText::new(t(d)).size(12.5).color(TEXT2));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| toggle(ui, on)).inner
    })
    .inner
}

/// Equal-width segments; returns the index clicked.
pub fn segmented(ui: &mut Ui, id: &str, labels: &[String], selected: usize, enabled: bool) -> Option<usize> {
    let n = labels.len().max(1);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, radius(12.0), SURFACE2);
    let w = rect.width() / n as f32;
    let pos = ui.ctx().animate_value_with_time(ui.id().with(id).with("seg"), selected as f32, 0.15);
    let sel = Rect::from_min_size(pos2(rect.left() + pos * w, rect.top()), vec2(w, rect.height())).shrink(3.0);
    p.rect_filled(sel, radius(9.0), if enabled { RED } else { RED_DEEP });
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * w, rect.top()), vec2(w, rect.height()));
        let resp = ui.interact(r, ui.id().with(id).with(i), if enabled { Sense::click() } else { Sense::hover() });
        let color = if i == selected {
            TEXT
        } else if resp.hovered() && enabled {
            TEXT
        } else {
            TEXT2
        };
        p.text(r.center(), Align2::CENTER_CENTER, t(label), font(14.0), color);
        if resp.clicked() {
            clicked = Some(i);
        }
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    clicked
}

#[derive(Clone, Copy, PartialEq)]
pub enum BigKind {
    Start,
    Stop,
}

/// The full-width call to action in a hero card.
pub fn big_button(ui: &mut Ui, g: &Gradients, text: &str, kind: BigKind) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    match kind {
        BigKind::Start => {
            let tint = mix(Color32::from_gray(235), Color32::WHITE, hover);
            gradient_rect(ui, rect, 14.0, &g.horizontal, Gradients::UV_H, tint);
        }
        BigKind::Stop => {
            ui.painter().rect(
                rect,
                radius(14.0),
                mix(RED_DEEP.gamma_multiply(0.55), RED_DEEP, hover),
                Stroke::new(1.5, RED),
                StrokeKind::Inside,
            );
        }
    }
    let galley = ui.painter().layout_no_wrap(t(text), font(17.0), Color32::WHITE);
    let icon_w = 16.0;
    let total = icon_w + 10.0 + galley.size().x;
    let x0 = rect.center().x - total / 2.0;
    let icon = Rect::from_center_size(pos2(x0 + icon_w / 2.0, rect.center().y), vec2(icon_w, icon_w));
    let p = ui.painter();
    match kind {
        BigKind::Start => {
            // Play triangle.
            let pts = vec![
                pos2(icon.left() + 2.0, icon.top()),
                pos2(icon.right(), icon.center().y),
                pos2(icon.left() + 2.0, icon.bottom()),
            ];
            p.add(Shape::convex_polygon(pts, Color32::WHITE, Stroke::NONE));
        }
        BigKind::Stop => {
            p.rect_filled(icon.shrink(1.5), radius(3.0), Color32::WHITE);
        }
    }
    p.galley(pos2(x0 + icon_w + 10.0, rect.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The letter to show in an avatar: the first letter or digit, skipping Thai
/// leading vowels (เ แ โ ใ ไ) so "เครื่องเล่นเกม" gives "ค".
pub fn initial(name: &str) -> String {
    if name.starts_with(|c: char| c.is_ascii_digit()) && name.contains('.') {
        return "IP".into();
    }
    name.chars()
        .find(|c| c.is_alphanumeric() && !('\u{0E40}'..='\u{0E44}').contains(c))
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// Round avatar with the first letter of `name`.
fn avatar(ui: &Ui, center: Pos2, name: &str, on: bool) {
    let p = ui.painter();
    p.circle_filled(center, 18.0, if on { RED } else { OUTLINE });
    let i = initial(name);
    let size = if i.chars().count() > 1 { 12.0 } else { 16.0 };
    p.text(center, Align2::CENTER_CENTER, t(i), font(size), Color32::WHITE);
}

fn check_circle(ui: &Ui, center: Pos2, on: f32) {
    let p = ui.painter();
    p.circle(center, 11.0, RED.gamma_multiply(on), Stroke::new(1.5, mix(TEXT3, RED, on)));
    if on > 0.01 {
        let c = Color32::WHITE.gamma_multiply(on);
        p.line_segment([center + vec2(-5.0, 0.0), center + vec2(-1.5, 3.5)], Stroke::new(2.0, c));
        p.line_segment([center + vec2(-1.5, 3.5), center + vec2(5.0, -3.5)], Stroke::new(2.0, c));
    }
}

/// A device in a pick list: avatar, name and detail, check circle.
pub fn device_row(ui: &mut Ui, name: &str, detail: &str, detail_color: Color32, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
    let k = ui.ctx().animate_bool_with_time(resp.id, selected, 0.14);
    let hover = resp.hovered();
    let fill = mix(if hover { Color32::from_rgb(0x23, 0x23, 0x29) } else { SURFACE2 }, RED.gamma_multiply(0.16), k);
    ui.painter().rect(rect, radius(12.0), fill, Stroke::new(1.0, mix(Color32::TRANSPARENT, RED, k)), StrokeKind::Inside);
    avatar(ui, pos2(rect.left() + 30.0, rect.center().y), name, selected);
    let x = rect.left() + 58.0;
    let max_w = rect.width() - 58.0 - 44.0;
    let p = ui.painter();
    let name_g = p.layout(t(name), font(15.0), TEXT, max_w);
    let detail_g = p.layout(t(detail), font(12.5), detail_color, max_w);
    let h = name_g.size().y + 2.0 + detail_g.size().y;
    let y = rect.center().y - h / 2.0;
    p.galley(pos2(x, y), name_g.clone(), TEXT);
    p.galley(pos2(x, y + name_g.size().y + 2.0), detail_g, detail_color);
    check_circle(ui, pos2(rect.right() - 26.0, rect.center().y), k);
    ui.add_space(-2.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VizStyle {
    Bars,
    Wave,
}

/// Visualizer with its own smoothing state (one per tap).
#[derive(Default)]
pub struct Viz {
    bars: Vec<f32>,
    last: Option<Instant>,
}

impl Viz {
    pub fn show(&mut self, ui: &mut Ui, snap: Option<&ScopeSnapshot>, style: VizStyle, height: f32) -> Response {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        let now = Instant::now();
        let dt = self.last.map(|l| now.duration_since(l).as_secs_f32()).unwrap_or(0.0).min(0.1);
        self.last = Some(now);
        let (n, gap) = match style {
            VizStyle::Bars => (48usize, 3.0f32),
            VizStyle::Wave => (((rect.width() / 4.0) as usize).max(16), 1.5f32),
        };
        let target: Vec<f32> = match (snap, style) {
            (Some(s), VizStyle::Bars) => s.spectrum(n),
            (Some(s), VizStyle::Wave) => s.waveform(n).into_iter().map(|v| v.sqrt()).collect(),
            (None, _) => vec![0.0; n],
        };
        if self.bars.len() != n || style == VizStyle::Wave {
            self.bars = target.clone();
        } else {
            // Rise instantly, fall at 2.5 heights per second.
            for (b, v) in self.bars.iter_mut().zip(&target) {
                *b = v.max(*b - dt * 2.5);
            }
        }
        let p = ui.painter();
        let cy = rect.center().y;
        let w = (rect.width() - gap * (n - 1) as f32) / n as f32;
        let half = rect.height() / 2.0 - 2.0;
        for (i, v) in self.bars.iter().enumerate() {
            let x = rect.left() + i as f32 * (w + gap);
            let h = (v * half).max(1.5);
            let quiet = *v < 0.03;
            let color = if quiet { TEXT3.gamma_multiply(0.7) } else { mix(RED, RED_HI, *v) };
            let r = (w / 2.0).min(4.0) as u8;
            let up = Rect::from_min_max(pos2(x, cy - h), pos2(x + w, cy));
            let down = Rect::from_min_max(pos2(x, cy), pos2(x + w, cy + h));
            p.rect_filled(up, CornerRadius { nw: r, ne: r, sw: 0, se: 0 }, color);
            p.rect_filled(down, CornerRadius { nw: 0, ne: 0, sw: r, se: r }, color.gamma_multiply(0.3));
        }
        if snap.is_some() {
            ui.ctx().request_repaint();
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(t("คลิกเพื่อสลับ ความถี่ / คลื่นเสียง"))
    }
}

/// Loudest volume the sliders reach (200 %).
pub const MAX_VOLUME: f32 = 2.0;
/// Share of a volume slider's length that covers 0–100 %; the rest is 100–200 %.
const UNITY_POS: f32 = 0.75;

/// Where a volume (0..=2) sits along a slider (0..=1).
pub fn volume_to_pos(v: f32) -> f32 {
    let v = v.clamp(0.0, MAX_VOLUME);
    if v <= 1.0 {
        v * UNITY_POS
    } else {
        UNITY_POS + (v - 1.0) / (MAX_VOLUME - 1.0) * (1.0 - UNITY_POS)
    }
}

/// The volume at a point along a slider (0..=1).
pub fn pos_to_volume(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    if p <= UNITY_POS {
        p / UNITY_POS
    } else {
        1.0 + (p - UNITY_POS) / (1.0 - UNITY_POS) * (MAX_VOLUME - 1.0)
    }
}

/// Volume control drawn on the level meter, full width. `level` is the
/// level before volume (0..1); returns a response that is `changed()` when moved.
pub fn volume_tube(ui: &mut Ui, g: &Gradients, volume: &mut f32, level: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click_and_drag());
    volume_tube_at(ui, g, rect, resp, volume, level)
}

/// The same control in a rect the caller has already claimed (the smaller
/// per-device slider). The mouse wheel is deliberately ignored, so scrolling
/// the page never changes the volume.
pub fn volume_tube_at(ui: &Ui, g: &Gradients, rect: Rect, mut resp: Response, volume: &mut f32, level: f32) -> Response {
    let knob_r = (rect.height() * 0.37).round();
    let x0 = rect.left() + knob_r;
    let x1 = rect.right() - knob_r;
    if resp.clicked() || resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let at = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            // Gentle snap to 100 %.
            let v = if (at - UNITY_POS).abs() < 0.02 { 1.0 } else { pos_to_volume(at) };
            if (v - *volume).abs() > f32::EPSILON {
                *volume = v;
                resp.mark_changed();
            }
        }
    }
    let kx = egui::lerp(x0..=x1, volume_to_pos(*volume));
    let p = ui.painter();
    let track = rect.shrink2(vec2(0.0, 1.0));
    p.rect(track, radius(track.height() / 2.0), SURFACE2, Stroke::new(1.0, OUTLINE), StrokeKind::Inside);
    // The volume's share of the tube, faintly, so it reads even in silence.
    let inner = track.shrink((track.height() * 0.14).round());
    let vol_rect = Rect::from_min_max(inner.min, pos2(kx.max(inner.left() + inner.height()), inner.bottom()));
    p.rect_filled(vol_rect, radius(inner.height() / 2.0), RED.gamma_multiply(0.14));
    // Live level, filling up to the knob at most.
    let lx = inner.left() + (kx - inner.left()) * level.clamp(0.0, 1.0);
    if lx > inner.left() + 2.0 {
        let lr = Rect::from_min_max(inner.min, pos2(lx.max(inner.left() + inner.height()), inner.bottom()));
        gradient_rect(ui, lr, inner.height() / 2.0, &g.horizontal, Gradients::UV_H, Color32::WHITE);
    }
    // 100 % mark.
    let x100 = egui::lerp(x0..=x1, UNITY_POS);
    let inset = track.height() * 0.23;
    p.line_segment(
        [pos2(x100, track.top() + inset), pos2(x100, track.bottom() - inset)],
        Stroke::new(1.5, Color32::from_white_alpha(70)),
    );
    let c = pos2(kx, rect.center().y);
    let grow = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || resp.dragged(), 0.1);
    p.circle_filled(c + vec2(0.0, 1.5), knob_r + 1.5, Color32::from_black_alpha(90));
    p.circle_filled(c, knob_r + grow * 1.5, RED);
    p.circle_filled(c, knob_r + grow * 1.5 - knob_r * 0.27, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---- icons -------------------------------------------------------------------

fn arc(center: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    (0..=16).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / 16.0;
        center + vec2(a.cos(), a.sin()) * r
    }).collect()
}

/// Speaker; `level` 0 draws it muted (with a cross).
pub fn speaker_icon(ui: &Ui, rect: Rect, color: Color32, muted: bool, level: f32) {
    let p = ui.painter();
    let c = rect.center();
    let s = rect.height() / 20.0;
    let body = vec![
        c + vec2(-8.0, -3.0) * s,
        c + vec2(-4.0, -3.0) * s,
        c + vec2(1.0, -7.5) * s,
        c + vec2(1.0, 7.5) * s,
        c + vec2(-4.0, 3.0) * s,
        c + vec2(-8.0, 3.0) * s,
    ];
    p.add(Shape::convex_polygon(body, color, Stroke::NONE));
    let st = Stroke::new(1.8 * s, color);
    if muted {
        p.line_segment([c + vec2(4.5, -3.5) * s, c + vec2(11.0, 3.5) * s], st);
        p.line_segment([c + vec2(4.5, 3.5) * s, c + vec2(11.0, -3.5) * s], st);
    } else {
        let a = std::f32::consts::FRAC_PI_4;
        p.add(Shape::line(arc(c + vec2(1.0, 0.0) * s, 5.0 * s, -a, a), st));
        if level > 0.66 {
            p.add(Shape::line(arc(c + vec2(1.0, 0.0) * s, 9.5 * s, -a, a), st));
        }
    }
}

pub fn send_icon(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let st = Stroke::new(2.0, color);
    p.circle_filled(c + vec2(0.0, 3.0), 2.5, color);
    let a = std::f32::consts::FRAC_PI_4;
    let up = -std::f32::consts::FRAC_PI_2;
    p.add(Shape::line(arc(c + vec2(0.0, 3.0), 6.0, up - a, up + a), st));
    p.add(Shape::line(arc(c + vec2(0.0, 3.0), 10.5, up - a, up + a), st));
}

pub fn receive_icon(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let st = Stroke::new(2.0, color);
    // Headphones.
    p.add(Shape::line(arc(c + vec2(0.0, 1.0), 7.5, std::f32::consts::PI, std::f32::consts::TAU), st));
    for dx in [-7.5f32, 7.5] {
        let r = Rect::from_center_size(c + vec2(dx, 4.0), vec2(4.5, 8.0));
        p.rect_filled(r, radius(2.0), color);
    }
}

pub fn settings_icon(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let d = vec2(a.cos(), a.sin());
        p.line_segment([c + d * 5.5, c + d * 9.0], Stroke::new(3.2, color));
    }
    p.circle_stroke(c, 6.0, Stroke::new(2.2, color));
}

/// Circular arrow.
pub fn refresh_icon(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let r = rect.height() * 0.36;
    let st = Stroke::new(2.0, color);
    let a0 = -std::f32::consts::FRAC_PI_2 + 0.5;
    let a1 = a0 + std::f32::consts::TAU - 1.1;
    p.add(Shape::line(arc(c, r, a0, a1), st));
    let tip = c + vec2(a0.cos(), a0.sin()) * r;
    let d = vec2(-a0.sin(), a0.cos()); // tangent at the start, pointing back along the arc
    let n = vec2(a0.cos(), a0.sin());
    let pts = vec![tip - d * 4.5, tip + n * 3.5 + d * 1.0, tip - n * 3.5 + d * 1.0];
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// An X.
pub fn close_icon(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let r = rect.shrink(rect.width() * 0.2);
    let st = Stroke::new(2.0, color);
    p.line_segment([r.left_top(), r.right_bottom()], st);
    p.line_segment([r.right_top(), r.left_bottom()], st);
}

/// Two overlapping sheets.
pub fn copy_icon(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width() * 0.55;
    let back = Rect::from_min_size(rect.min + vec2(rect.width() * 0.1, rect.height() * 0.1), vec2(s, s));
    let front = Rect::from_min_size(rect.min + vec2(rect.width() * 0.35, rect.height() * 0.35), vec2(s, s));
    p.rect_stroke(back, radius(2.5), Stroke::new(1.6, color.gamma_multiply(0.6)), StrokeKind::Inside);
    p.rect(front, radius(2.5), SURFACE2, Stroke::new(1.6, color), StrokeKind::Inside);
}

/// Logo mark: gradient rounded square with five equalizer bars.
pub fn logo(ui: &mut Ui, g: &Gradients, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    gradient_rect(ui, rect, size * 0.24, &g.horizontal, Gradients::UV_H, Color32::WHITE);
    // Same bars as the app icon (assets/icon.png).
    let heights = [0.205f32, 0.41, 0.586, 0.352, 0.234];
    let bw = size * 0.09;
    let gap = size * 0.062;
    let total = 5.0 * bw + 4.0 * gap;
    let p = ui.painter();
    for (i, h) in heights.iter().enumerate() {
        let x = rect.center().x - total / 2.0 + i as f32 * (bw + gap);
        let r = Rect::from_min_size(pos2(x, rect.center().y - h * size / 2.0), vec2(bw, h * size));
        p.rect_filled(r, radius(bw / 2.0), Color32::WHITE);
    }
    resp
}

/// A round icon button.
pub fn icon_button(ui: &mut Ui, size: f32, draw: impl FnOnce(&Ui, Rect, Color32)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
    ui.painter().circle_filled(rect.center(), size / 2.0, mix(SURFACE2, Color32::from_rgb(0x2C, 0x2C, 0x34), hover));
    draw(ui, rect.shrink(size * 0.22), TEXT);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Small text button in the accent colour.
pub fn link_button(ui: &mut Ui, text: &str) -> Response {
    let r = ui.add(egui::Label::new(RichText::new(t(text)).size(13.5).color(RED_HI)).sense(Sense::click()));
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_slider_curve() {
        // 75 % of the length is 0–100 %, the last 25 % is 100–200 %.
        assert_eq!(volume_to_pos(0.0), 0.0);
        assert_eq!(volume_to_pos(0.5), 0.375);
        assert_eq!(volume_to_pos(1.0), 0.75);
        assert_eq!(volume_to_pos(1.5), 0.875);
        assert_eq!(volume_to_pos(2.0), 1.0);
        assert_eq!(volume_to_pos(9.0), 1.0);
        for i in 0..=100 {
            let p = i as f32 / 100.0;
            assert!((volume_to_pos(pos_to_volume(p)) - p).abs() < 1e-6);
        }
        assert!((pos_to_volume(0.76) - 1.04).abs() < 1e-5, "just past 100 % moves 4 % per 1 % of length");
    }
}
