//! Settings tab: sound mode, this device, adding devices by IP, display,
//! and updates.

use super::*;
use eframe::egui::{Stroke, StrokeKind, Ui};
use ssnd_core::Mode;

const MODES: [(Mode, &str, &str, &str); 3] = [
    (Mode::Game, "เกม", "หน่วงต่ำสุด", "เสียงตรงกับภาพที่สุด เสียงที่มาช้าจะถูกข้ามไป ถ้า Wi-Fi สะดุดอาจได้ยินเสียงแตกสั้น ๆ"),
    (Mode::Balanced, "สมดุล", "แนะนำ", "หน่วงต่ำและไม่สะดุด เหมาะกับการใช้งานทั่วไป"),
    (Mode::Music, "ฟังเพลง", "เสถียรที่สุด", "เผื่อเวลามากขึ้น ทน Wi-Fi ที่ไม่เสถียรได้ดีที่สุด ไม่ทิ้งเสียงเลย"),
];

pub fn show(app: &mut App, ui: &mut Ui) {
    mode_card(app, ui);
    device_card(app, ui);
    manual_card(app, ui);
    general_card(app, ui);
    update_card(app, ui);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(t("Stream Sound · ส่งเสียงข้ามเครื่องในวง LAN")).size(12.0).color(TEXT3));
    });
}

fn mode_card(app: &mut App, ui: &mut Ui) {
    let mut picked = None;
    card(ui, |ui| {
        section_title(ui, "โหมดเสียง");
        hint(ui, "ใช้กับเสียงที่เครื่องนี้รับ และขนาดแพ็กเก็ตที่เครื่องนี้ส่ง");
        ui.add_space(6.0);
        for (m, name, tag, detail) in MODES {
            if mode_option(ui, name, tag, detail, app.s.mode == m).clicked() {
                picked = Some(m);
            }
            ui.add_space(6.0);
        }
    });
    if let Some(m) = picked.filter(|m| *m != app.s.mode) {
        app.s.mode = m;
        app.engine.set_mode(m);
        app.save();
    }
}

/// A radio card: title, tag, description, and a ring that fills when chosen.
fn mode_option(ui: &mut Ui, name: &str, tag: &str, detail: &str, on: bool) -> egui::Response {
    let w = ui.available_width();
    let text_w = w - 16.0 - 36.0 - 16.0;
    let p = ui.painter().clone();
    let name_g = p.layout_no_wrap(t(name), font(15.5), TEXT);
    let tag_g = p.layout_no_wrap(t(tag), font(12.0), if on { RED_HI } else { TEXT2 });
    let detail_g = p.layout(t(detail), font(12.5), TEXT2, text_w);
    let h = 14.0 + name_g.size().y.max(20.0) + 3.0 + detail_g.size().y + 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let k = ui.ctx().animate_bool_with_time(resp.id, on, 0.14);
    let fill = mix(if resp.hovered() { Color32::from_rgb(0x23, 0x23, 0x29) } else { SURFACE2 }, RED.gamma_multiply(0.14), k);
    p.rect(rect, 12, fill, Stroke::new(1.0, mix(Color32::TRANSPARENT, RED, k)), StrokeKind::Inside);
    // Radio ring.
    let c = pos2(rect.left() + 26.0, rect.top() + 14.0 + name_g.size().y.max(20.0) / 2.0);
    p.circle_stroke(c, 9.0, Stroke::new(1.8, mix(TEXT3, RED, k)));
    if k > 0.01 {
        p.circle_filled(c, 5.0 * k, RED_HI);
    }
    let x = rect.left() + 46.0;
    let y = rect.top() + 14.0;
    let name_w = name_g.size().x;
    let name_h = name_g.size().y.max(20.0);
    p.galley(pos2(x, c.y - name_g.size().y / 2.0), name_g, TEXT);
    let tag_rect = Rect::from_min_size(pos2(x + name_w + 8.0, c.y - 10.0), vec2(tag_g.size().x + 14.0, 20.0));
    p.rect_filled(tag_rect, 10, if on { RED.gamma_multiply(0.22) } else { OUTLINE });
    p.galley(pos2(tag_rect.left() + 7.0, tag_rect.center().y - tag_g.size().y / 2.0), tag_g, TEXT2);
    p.galley(pos2(x, y + name_h + 3.0), detail_g, TEXT2);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn device_card(app: &mut App, ui: &mut Ui) {
    let mut copy: Option<String> = None;
    let mut commit = false;
    card(ui, |ui| {
        section_title(ui, "เครื่องนี้");
        ui.add_space(4.0);
        ui.label(RichText::new(t("ชื่อที่เครื่องอื่นเห็น")).size(13.0).color(TEXT2));
        let resp = ui.add(
            egui::TextEdit::singleline(&mut app.name_input)
                .desired_width(f32::INFINITY)
                .char_limit(60)
                .margin(vec2(12.0, 9.0))
                .font(font(15.0))
                .hint_text(RichText::new(t("ชื่อเครื่อง")).color(TEXT3)),
        );
        commit = resp.lost_focus();
        ui.add_space(10.0);
        ui.label(RichText::new(t("IP ของเครื่องนี้ (ให้เครื่องอื่นส่งมาที่นี่)")).size(13.0).color(TEXT2));
        ui.add_space(2.0);
        if app.ips.is_empty() {
            ui.label(RichText::new(t("ยังไม่ได้ต่อ Wi-Fi หรือสาย LAN")).size(14.0).color(AMBER));
        }
        ui.horizontal_wrapped(|ui| {
            for ip in &app.ips {
                if ip_chip(ui, &ip.to_string()).clicked() {
                    copy = Some(ip.to_string());
                }
            }
        });
        ui.add_space(2.0);
        hint(ui, &format!("พอร์ต {DEFAULT_AUDIO_PORT} (UDP) · ค้นหาเครื่องอัตโนมัติผ่านพอร์ต {}", DEFAULT_AUDIO_PORT + 1));
    });
    if commit {
        let name = app.name_input.trim().to_string();
        if name.is_empty() {
            app.name_input = app.engine.name();
        } else if name != app.engine.name() {
            app.engine.set_name(&name);
            app.name_input = app.engine.name();
            app.s.name = Some(app.name_input.clone());
            app.save();
            app.toast(t("เปลี่ยนชื่อแล้ว เครื่องอื่นจะเห็นในไม่กี่วินาที"), false);
        }
    }
    if let Some(ip) = copy {
        ui.ctx().copy_text(ip.clone());
        app.toast(t(format!("คัดลอก {ip} แล้ว")), false);
    }
}

fn ip_chip(ui: &mut Ui, ip: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(ip.to_string(), font(15.0), TEXT);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 50.0, 36.0), Sense::click());
    let hover = resp.hovered();
    ui.painter().rect(rect, 10, if hover { OUTLINE } else { SURFACE2 }, Stroke::new(1.0, OUTLINE), StrokeKind::Inside);
    ui.painter().galley(pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, TEXT);
    let icon = Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(16.0, 16.0));
    copy_icon(ui, icon, if hover { TEXT } else { TEXT2 });
    resp.on_hover_text(t("คลิกเพื่อคัดลอก")).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn manual_card(app: &mut App, ui: &mut Ui) {
    let mut add = false;
    let mut remove = None;
    card(ui, |ui| {
        section_title(ui, "เพิ่มเครื่องด้วย IP");
        hint(ui, "ใช้เมื่อเครื่องไม่ขึ้นเอง เช่น อยู่คนละวง Wi-Fi หรือเราเตอร์บล็อกการค้นหา");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let w = ui.available_width() - 78.0;
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.manual_input)
                    .desired_width(w)
                    .margin(vec2(12.0, 9.0))
                    .font(font(15.0))
                    .hint_text(RichText::new("192.168.1.20").color(TEXT3)),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                add = true;
            }
            let b = egui::Button::new(RichText::new(t("เพิ่ม")).size(15.0).color(Color32::WHITE))
                .fill(RED)
                .corner_radius(10)
                .min_size(vec2(70.0, 36.0));
            if ui.add(b).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                add = true;
            }
        });
        if !app.s.manual.is_empty() {
            ui.add_space(6.0);
        }
        for (i, m) in app.s.manual.iter().enumerate() {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::hover());
            ui.painter().rect_filled(rect, 10, SURFACE2);
            let ok = resolve(m).is_some();
            ui.painter().text(pos2(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, m, font(15.0), TEXT);
            if !ok {
                ui.painter().text(
                    pos2(rect.right() - 48.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    t("หาไม่เจอ"),
                    font(12.5),
                    AMBER,
                );
            }
            let btn = Rect::from_center_size(pos2(rect.right() - 22.0, rect.center().y), vec2(30.0, 30.0));
            let resp = ui.interact(btn, egui::Id::new("manual-rm").with(i), Sense::click());
            if resp.hovered() {
                ui.painter().circle_filled(btn.center(), 15.0, OUTLINE);
            }
            close_icon(ui, btn.shrink(8.0), if resp.hovered() { TEXT } else { TEXT2 });
            if resp.on_hover_text(t("ลบ")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                remove = Some(i);
            }
            ui.add_space(4.0);
        }
    });
    if add {
        let v = app.manual_input.trim().to_string();
        match resolve(&v) {
            _ if v.is_empty() => {}
            Some(a) => {
                if !app.s.manual.contains(&v) {
                    app.s.manual.push(v);
                }
                let key = a.to_string();
                if !app.s.send_to.contains(&key) {
                    app.s.send_to.push(key);
                }
                app.save();
                app.manual_input.clear();
                app.toast(t(format!("เพิ่ม {a} แล้ว และเลือกเป็นปลายทางส่งเสียง")), false);
            }
            None => app.toast(t(format!("ไม่รู้จักที่อยู่ \"{v}\" ลองพิมพ์แบบ 192.168.1.20")), true),
        }
    }
    if let Some(i) = remove {
        let m = app.s.manual.remove(i);
        if let Some(a) = resolve(&m) {
            let key = a.to_string();
            if !app.peers.iter().any(|p| p.addr() == a) {
                app.s.send_to.retain(|k| *k != key);
                app.s.forward_to.retain(|k| *k != key);
            }
        }
        app.save();
    }
}

fn general_card(app: &mut App, ui: &mut Ui) {
    let mut visual_changed = false;
    let mut auto_changed = false;
    card(ui, |ui| {
        section_title(ui, "ทั่วไป");
        ui.add_space(6.0);
        ui.label(RichText::new(t("ภาพเสียง")).size(13.0).color(TEXT2));
        ui.add_space(2.0);
        let labels = vec!["แท่งความถี่".to_string(), "คลื่นเสียง".to_string()];
        let sel = if app.s.visual == Visual::Wave { 1 } else { 0 };
        if let Some(i) = segmented(ui, "visual", &labels, sel, true) {
            let v = if i == 1 { Visual::Wave } else { Visual::Bars };
            visual_changed = v != app.s.visual;
            app.s.visual = v;
        }
        ui.add_space(10.0);
        auto_changed = toggle_row(
            ui,
            "เปิดรับเสียงทันทีเมื่อเปิดแอป",
            Some("เครื่องอื่นส่งมาได้เลยโดยไม่ต้องกดอะไร"),
            &mut app.s.auto_receive,
        )
        .changed();
    });
    if visual_changed || auto_changed {
        app.save();
    }
}

fn update_card(app: &mut App, ui: &mut Ui) {
    let status = app.update.lock().clone();
    let build = updater::current_build();
    let mut check = false;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - 150.0);
                section_title(ui, "อัปเดต");
                let ver = if build == 0 { "รุ่นทดสอบ (dev)".to_string() } else { format!("build {build}") };
                ui.label(RichText::new(t(format!("เวอร์ชันนี้: {ver}"))).size(13.0).color(TEXT2));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let idle = matches!(
                    status,
                    UpdateStatus::Idle | UpdateStatus::UpToDate | UpdateStatus::NoRelease | UpdateStatus::Failed(_)
                );
                let b = egui::Button::new(RichText::new(t("ตรวจสอบอัปเดต")).size(14.0).color(if idle { TEXT } else { TEXT3 }))
                    .fill(SURFACE2)
                    .stroke(Stroke::new(1.0, OUTLINE))
                    .corner_radius(10)
                    .min_size(vec2(0.0, 36.0));
                if ui.add_enabled(idle, b).clicked() {
                    check = true;
                }
            });
        });
        let (text, color) = match &status {
            UpdateStatus::Idle => (String::new(), TEXT2),
            UpdateStatus::Checking => ("กำลังตรวจสอบ…".to_string(), TEXT2),
            UpdateStatus::UpToDate => ("เป็นเวอร์ชันล่าสุดแล้ว".to_string(), GREEN),
            UpdateStatus::NoRelease => ("ยังไม่มีเวอร์ชันที่เผยแพร่บน GitHub".to_string(), TEXT2),
            UpdateStatus::Downloading { release, percent } => {
                (format!("กำลังดาวน์โหลด build {} ({percent}%)", release.build), TEXT)
            }
            UpdateStatus::Ready { release, .. } => (format!("build {} พร้อมติดตั้ง กดปุ่มด้านบนสุดเพื่ออัปเดต", release.build), RED_HI),
            UpdateStatus::Failed(e) => (update_error(e), ERROR),
        };
        if !text.is_empty() {
            ui.add_space(6.0);
            if let UpdateStatus::Downloading { percent, .. } = &status {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                ui.painter().rect_filled(r, 3, SURFACE2);
                let f = Rect::from_min_size(r.min, vec2(r.width() * *percent as f32 / 100.0, r.height()));
                ui.painter().rect_filled(f, 3, RED);
                ui.add_space(4.0);
            }
            let r = ui.label(RichText::new(t(text)).size(13.0).color(color));
            if let UpdateStatus::Failed(e) = &status {
                r.on_hover_text(e);
            }
        }
        hint(ui, "แอปตรวจหาเวอร์ชันใหม่จาก GitHub ให้เองทุก 6 ชั่วโมง");
    });
    if check {
        app.last_update_check = Instant::now();
        *app.update.lock() = UpdateStatus::Idle;
        updater::check_and_download_in_background(app.update.clone());
    }
}

/// Short Thai reason for a failed update check; the full error is on hover.
fn update_error(e: &str) -> String {
    let l = e.to_lowercase();
    if ["connection", "dns", "resolve", "timed out", "timeout", "tls", "certificate", "network"].iter().any(|k| l.contains(k)) {
        "ติดต่อ GitHub ไม่ได้ ตรวจการเชื่อมต่ออินเทอร์เน็ตแล้วลองใหม่".into()
    } else if l.contains("403") || l.contains("429") {
        "GitHub จำกัดการตรวจสอบชั่วคราว ลองใหม่ภายหลัง".into()
    } else {
        "ตรวจสอบอัปเดตไม่สำเร็จ ลองใหม่ภายหลัง".into()
    }
}
