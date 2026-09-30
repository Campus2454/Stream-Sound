//! Receive tab: what's playing (visualizer, on/off, volume tube), who it
//! comes from, and relaying it on to other devices.

use super::*;
use eframe::egui::{Stroke, StrokeKind, Ui};
use ssnd_core::{StreamStats, Tap};

pub fn show(app: &mut App, ui: &mut Ui) {
    let streams = app.engine.streams();
    track_stutter(app, &streams);
    hero(app, ui, &streams);
    streams_card(app, ui, &streams);
    relay_card(app, ui);
}

/// Remember when each stream last stuttered, to colour its delay pill.
fn track_stutter(app: &mut App, streams: &[StreamStats]) {
    for s in streams {
        let e = app.stutter.entry(s.id).or_insert((s.underruns, None));
        if s.underruns > e.0 {
            *e = (s.underruns, Some(Instant::now()));
        }
    }
    app.stutter.retain(|id, _| streams.iter().any(|s| s.id == *id));
}

fn hero(app: &mut App, ui: &mut Ui, streams: &[StreamStats]) {
    let receiving = app.engine.is_receiving();
    let snap = app.engine.scope(Tap::Receive);
    // Level before volume, for the tube: newest 15 ms of the played waveform.
    let played = snap.as_ref().map(|s| s.waveform(400)[397..].iter().fold(0.0f32, |m, v| m.max(*v))).unwrap_or(0.0);
    let vol = if app.s.muted { 0.0 } else { app.s.volume };
    let pre = if vol > 0.01 { (played / vol).min(1.0) } else { 0.0 };
    app.tube_level = if pre > app.tube_level { pre } else { app.tube_level * 0.88 + pre * 0.12 };
    let mut switch = receiving;
    let mut switched = false;
    let mut flip = false;
    let mut vol_changed = false;
    let mut vol_done = false;
    let mut mute = false;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            if !receiving {
                pill(ui, "ปิดรับอยู่", TEXT3, true, false);
            } else if streams.is_empty() {
                pill(ui, "รอเสียง…", TEXT2, true, false);
            } else {
                let n = streams.len();
                let label = if n == 1 { "กำลังเล่น".to_string() } else { format!("กำลังเล่น · {n} แหล่ง") };
                pill(ui, &label, GREEN, true, true);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                switched = toggle(ui, &mut switch).changed();
                ui.label(RichText::new(t("เปิดรับ")).size(14.0).color(TEXT2));
            });
        });
        ui.add_space(8.0);
        flip = app.viz_recv.show(ui, snap.as_ref(), visual_style(app.s.visual), 120.0).clicked();
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let muted = app.s.muted;
            let level = volume_to_pos(app.s.volume);
            mute = icon_button(ui, 36.0, |ui, r, c| speaker_icon(ui, r, c, muted || app.s.volume == 0.0, level))
                .on_hover_text(t(if muted { "เปิดเสียง" } else { "ปิดเสียง" }))
                .clicked();
            let w = ui.available_width() - 54.0;
            let resp = ui
                .allocate_ui(vec2(w, 30.0), |ui| volume_tube(ui, &app.g, &mut app.s.volume, app.tube_level))
                .inner;
            vol_changed = resp.changed();
            vol_done = resp.drag_stopped() || (resp.changed() && !resp.dragged());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let txt = if app.s.muted { "ปิด".to_string() } else { format!("{:.0}%", app.s.volume * 100.0) };
                ui.label(RichText::new(t(txt)).size(14.0).color(if app.s.muted { TEXT3 } else { TEXT }));
            });
        });
        if let Some(e) = app.engine.receiver_error() {
            ui.add_space(4.0);
            ui.label(RichText::new(t("เปิดลำโพงไม่ได้ กำลังลองใหม่อัตโนมัติ… ตรวจว่าเสียบลำโพงหรือหูฟังอยู่"))
                .size(12.5)
                .color(ERROR))
                .on_hover_text(e);
        }
    });
    if flip {
        app.s.visual = if app.s.visual == Visual::Bars { Visual::Wave } else { Visual::Bars };
        app.save();
    }
    if switched {
        if switch {
            app.start_receiving();
        } else {
            app.engine.stop_receiving();
        }
    }
    if mute {
        app.s.muted = !app.s.muted;
        vol_changed = true;
        vol_done = true;
    }
    if vol_changed {
        if app.s.volume > 0.0 && !mute {
            app.s.muted = false;
        }
        app.engine.set_volume(if app.s.muted { 0.0 } else { app.s.volume });
    }
    if vol_done {
        app.save();
    }
}

fn streams_card(app: &mut App, ui: &mut Ui, streams: &[StreamStats]) {
    let out_ms = app.engine.output_latency_ms();
    let receiving = app.engine.is_receiving();
    let mut toggle_id = None;
    card(ui, |ui| {
        section_title(ui, "กำลังรับจาก");
        if streams.is_empty() {
            let msg = if receiving {
                match app.ips.first() {
                    Some(ip) => format!("ยังไม่มีเสียงเข้ามา ให้อีกเครื่องส่งมาที่ {ip}"),
                    None => "ยังไม่มีเสียงเข้ามา".to_string(),
                }
            } else {
                "เปิดรับเสียงก่อน แล้วเครื่องอื่นจะส่งมาที่เครื่องนี้ได้".to_string()
            };
            hint(ui, &msg);
            return;
        }
        ui.add_space(4.0);
        for s in streams {
            let recent = app.stutter.get(&s.id).and_then(|e| e.1).map(|t| t.elapsed() < Duration::from_secs(10)).unwrap_or(false);
            let open = app.expanded == Some(s.id);
            let row = stream_row(ui, s, s.capture_ms + s.buffer_ms + out_ms, recent, open);
            device_volume(app, ui, s, row.speaker);
            if row.resp.clicked() {
                toggle_id = Some(s.id);
            }
            if open {
                details(ui, s, out_ms);
            }
        }
        ui.add_space(2.0);
        hint(ui, "ตัวเลขไม่รวมเวลาเดินทางในเครือข่าย (สาย LAN ~1 ms, Wi-Fi ~2–10 ms)");
    });
    if let Some(id) = toggle_id {
        app.expanded = if app.expanded == Some(id) { None } else { Some(id) };
    }
}

struct Row {
    resp: egui::Response,
    /// Where the device's speaker button goes, left of the delay.
    speaker: Rect,
}

fn stream_row(ui: &mut Ui, s: &StreamStats, delay: f64, stutter: bool, open: bool) -> Row {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
    let hover = resp.hovered();
    let p = ui.painter().clone();
    p.rect(
        rect,
        12,
        if hover || open { Color32::from_rgb(0x23, 0x23, 0x29) } else { SURFACE2 },
        Stroke::NONE,
        StrokeKind::Inside,
    );
    // Live level ring around the avatar.
    let c = pos2(rect.left() + 30.0, rect.center().y);
    p.circle_filled(c, 18.0, OUTLINE);
    p.circle_stroke(c, 18.0, Stroke::new(2.5, GREEN.gamma_multiply(0.25 + 0.75 * s.level.clamp(0.0, 1.0))));
    p.text(c, Align2::CENTER_CENTER, t(initial(&s.name)), font(16.0), Color32::WHITE);
    // Just the delay, coloured by how long it is and by recent stutters.
    let color = delay_color(delay, stutter);
    // A fixed width, so the pill and the speaker beside it don't move as the
    // number changes.
    let pill_g = p.layout_no_wrap(delay_text(delay), font(13.0), color);
    let widest = p.layout_no_wrap(DELAY_TEXT_WIDEST.into(), font(13.0), color).size().x;
    let pill_w = pill_g.size().x.max(widest) + 20.0;
    let pill = Rect::from_min_size(pos2(rect.right() - 14.0 - pill_w, rect.center().y - 13.0), vec2(pill_w, 26.0));
    p.rect_filled(pill, 13, color.gamma_multiply(0.14));
    p.galley(pill.center() - pill_g.size() / 2.0, pill_g, color);
    let speaker = Rect::from_center_size(pos2(pill.left() - 22.0, rect.center().y), vec2(32.0, 32.0));
    let x = rect.left() + 58.0;
    let max_w = speaker.left() - x - 6.0;
    let from = s.from.split(':').next().unwrap_or(&s.from);
    let name_g = p.layout(t(&s.name), font(15.0), TEXT, max_w);
    let sub_g = p.layout(t(format!("จาก {from}")), font(12.5), TEXT2, max_w);
    let h = name_g.size().y + 2.0 + sub_g.size().y;
    let y = rect.center().y - h / 2.0;
    let name_h = name_g.size().y;
    p.galley(pos2(x, y), name_g, TEXT);
    p.galley(pos2(x, y + name_h + 2.0), sub_g, TEXT2);
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let resp = if ui.rect_contains_pointer(speaker) { resp } else { resp.on_hover_text(t("คลิกเพื่อดูรายละเอียด")) };
    Row { resp, speaker }
}

/// A device's own volume: a speaker button that opens a small slider while
/// the mouse is over it (and for a moment after, so it can be reached).
/// Clicking the speaker mutes or unmutes just that device.
fn device_volume(app: &mut App, ui: &mut Ui, s: &StreamStats, at: Rect) {
    const LINGER: Duration = Duration::from_millis(400);
    let ip = s.from.split(':').next().unwrap_or(&s.from).to_string();
    // By stream: one device can send more than one.
    let id = egui::Id::new("device-volume").with(s.id);
    let mut vol = app.s.source_volume(&ip);
    let resp = ui.interact(at, id, Sense::click());
    let mut keep = resp.hovered();
    let open_before = app.device_hover.is_some_and(|(k, t)| k == s.id && t.elapsed() < LINGER);
    let shown = ui.ctx().animate_bool_with_time(id.with("shown"), open_before || resp.hovered(), 0.12);
    let hover = resp.hovered() || open_before;
    let bg = if hover { Color32::from_rgb(0x2C, 0x2C, 0x34) } else { Color32::TRANSPARENT };
    ui.painter().circle_filled(at.center(), at.width() / 2.0, bg);
    let color = if vol == 0.0 { TEXT3 } else if vol != 1.0 { RED_HI } else { TEXT2 };
    speaker_icon(ui, at.shrink(8.0), color, vol == 0.0, volume_to_pos(vol));
    let mut changed = false;
    let mut done = false;
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        vol = if vol == 0.0 { app.device_unmute.get(&ip).copied().unwrap_or(1.0) } else { 0.0 };
        changed = true;
        done = true;
    }
    if shown > 0.0 {
        // Pops out to the left of the speaker, over the device's name.
        let size = vec2(214.0, 40.0);
        let pos = pos2(at.left() - size.x - 2.0, at.center().y - size.y / 2.0);
        egui::Area::new(id.with("pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(true)
            .show(ui.ctx(), |ui| {
                // Claim the space so the pointer counts as being over it.
                let (frame, _) = ui.allocate_exact_size(size, Sense::hover());
                let fill = Color32::from_rgb(0x2C, 0x2C, 0x34).gamma_multiply(shown);
                ui.painter().rect(frame, 20, fill, Stroke::new(1.0, OUTLINE.gamma_multiply(shown)), StrokeKind::Inside);
                let slider = Rect::from_min_size(frame.min + vec2(12.0, 9.0), vec2(150.0, 22.0));
                let sresp = ui.interact(slider, id.with("slider"), Sense::click_and_drag());
                let sresp = volume_tube_at(ui, &app.g, slider, sresp, &mut vol, s.level);
                changed |= sresp.changed();
                done |= sresp.drag_stopped() || (sresp.changed() && !sresp.dragged());
                keep |= sresp.dragged() || ui.rect_contains_pointer(frame);
                let txt = if vol == 0.0 { "ปิด".to_string() } else { format!("{:.0}%", vol * 100.0) };
                ui.painter().text(
                    pos2(frame.right() - 12.0, frame.center().y),
                    Align2::RIGHT_CENTER,
                    t(txt),
                    font(13.0),
                    (if vol == 0.0 { TEXT3 } else { TEXT }).gamma_multiply(shown),
                );
            });
    }
    if keep {
        app.device_hover = Some((s.id, Instant::now()));
    }
    if (shown > 0.0 && shown < 1.0) || open_before {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    if changed {
        if vol > 0.0 {
            app.device_unmute.insert(ip.clone(), vol);
        }
        app.s.set_source_volume(&ip, vol);
        app.engine.set_source_volume(&ip, vol);
    }
    if done {
        app.save();
    }
}

fn details(ui: &mut Ui, s: &StreamStats, out_ms: f64) {
    egui::Frame::new().inner_margin(egui::Margin { left: 58, right: 12, top: 2, bottom: 8 }).show(ui, |ui| {
        let mut parts = Vec::new();
        if s.capture_ms > 0.0 {
            parts.push(format!("จับเสียง {:.0}", s.capture_ms));
        }
        parts.push(format!("บัฟเฟอร์ {:.0}", s.buffer_ms));
        if out_ms > 0.0 {
            parts.push(format!("ลำโพง {:.0}", out_ms));
        }
        ui.label(RichText::new(t(format!("หน่วงรวม ~{:.0} ms = {}", s.capture_ms + s.buffer_ms + out_ms, parts.join(" + ")))).size(13.0).color(TEXT2));
        ui.label(
            RichText::new(t(format!(
                "หาย {} · มาช้า {} · สะดุด {} · {} kHz {}",
                s.lost,
                s.late,
                s.underruns,
                s.sample_rate as f32 / 1000.0,
                if s.channels == 1 { "โมโน" } else { "สเตอริโอ" }
            )))
            .size(13.0)
            .color(TEXT2),
        );
    });
}

fn relay_card(app: &mut App, ui: &mut Ui) {
    let targets = app.targets();
    let mut toggled: Option<String> = None;
    let mut open_click = false;
    let mut play_changed = false;
    let mut go_settings = false;
    card(ui, |ui| {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        let p = ui.painter().clone();
        p.text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, t("ส่งต่อ (ต่อเป็นทอด)"), font(16.0), TEXT);
        let n = app.s.forward_to.len();
        let k = ui.ctx().animate_bool_with_time(resp.id, app.relay_open, 0.15);
        let c = pos2(rect.right() - 8.0, rect.center().y);
        let (a, b, d) = if k > 0.5 {
            (c + vec2(-5.0, 2.5), c + vec2(0.0, -2.5), c + vec2(5.0, 2.5))
        } else {
            (c + vec2(-5.0, -2.5), c + vec2(0.0, 2.5), c + vec2(5.0, -2.5))
        };
        p.line_segment([a, b], Stroke::new(2.0, TEXT2));
        p.line_segment([b, d], Stroke::new(2.0, TEXT2));
        if n > 0 {
            p.text(pos2(rect.right() - 24.0, rect.center().y), Align2::RIGHT_CENTER, t(format!("{n} เครื่อง")), font(12.5), RED_HI);
        }
        open_click = resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
        if !app.relay_open {
            hint(ui, "ส่งเสียงที่รับได้ต่อไปยังเครื่องอื่นทันที เพิ่มหน่วงไม่ถึง 1 ms");
            return;
        }
        ui.add_space(4.0);
        play_changed = toggle_row(
            ui,
            "เล่นเสียงที่เครื่องนี้",
            Some("ปิดไว้ถ้าเครื่องนี้เป็นแค่ตัวส่งต่อ"),
            &mut app.s.play_local,
        )
        .changed();
        ui.add_space(4.0);
        if targets.is_empty() && app.s.forward_to.is_empty() {
            send::empty_devices(ui, &mut go_settings);
        }
        for tg in &targets {
            let on = app.s.forward_to.contains(&tg.key);
            let detail = if tg.receiving { tg.addr.ip().to_string() } else { format!("{} · ไม่ได้เปิดรับเสียง", tg.addr.ip()) };
            if device_row(ui, &tg.name, &detail, if tg.receiving { TEXT2 } else { AMBER }, on).clicked() {
                toggled = Some(tg.key.clone());
            }
        }
        for key in app.s.forward_to.iter().filter(|k| !targets.iter().any(|t| &t.key == *k)) {
            let ip = key.split(':').next().unwrap_or(key);
            if device_row(ui, ip, "ไม่พบในเครือข่ายตอนนี้", TEXT3, true).clicked() {
                toggled = Some(key.clone());
            }
        }
        let fwd = app.engine.forwarded_packets();
        if fwd > 0 {
            hint(ui, &format!("ส่งต่อแล้ว {fwd} แพ็กเก็ต"));
        }
    });
    if open_click {
        app.relay_open = !app.relay_open;
    }
    if go_settings {
        app.tab = Tab::Settings;
    }
    if play_changed {
        app.engine.set_play_local(app.s.play_local);
        app.save();
    }
    if let Some(k) = toggled {
        if let Some(i) = app.s.forward_to.iter().position(|x| *x == k) {
            app.s.forward_to.remove(i);
        } else {
            app.s.forward_to.push(k);
        }
        app.save();
    }
}
