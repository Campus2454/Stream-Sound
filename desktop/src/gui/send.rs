//! Send tab: what's being sent (visualizer + start/stop), the source, and
//! the devices to send to.

use super::*;
use eframe::egui::Ui;
use ssnd_core::{Source, SourceKind, Tap};

const KINDS: [(SourceKind, &str, &str); 4] = [
    (SourceKind::System, "ทั้งเครื่อง", "ส่งทุกเสียงที่เครื่องนี้เล่นอยู่"),
    (SourceKind::App, "เฉพาะแอป", "ส่งเสียงจากแอปเดียว เช่น เกมหรือเบราว์เซอร์"),
    (SourceKind::Input, "อินพุต", "ไมโครโฟนหรือสายสัญญาณเข้า"),
    (SourceKind::Tone, "เสียงทดสอบ", "โทน 440 Hz ไว้ทดสอบการเชื่อมต่อ"),
];

fn kind_of(s: &Source) -> SourceKind {
    match s {
        Source::App { .. } => SourceKind::App,
        Source::Input { .. } => SourceKind::Input,
        Source::Tone => SourceKind::Tone,
        Source::System | Source::External => SourceKind::System,
    }
}

/// "แอป: Chrome" → "Chrome".
fn short_label(label: &str) -> &str {
    label.split_once(": ").map(|(_, r)| r).unwrap_or(label)
}

pub fn show(app: &mut App, ui: &mut Ui) {
    hero(app, ui);
    source_card(app, ui);
    targets_card(app, ui);
}

fn hero(app: &mut App, ui: &mut Ui) {
    let st = app.engine.sender_stats();
    let snap = app.engine.scope(Tap::Send);
    let mut start = false;
    let mut stop = false;
    let mut flip = false;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            if st.active {
                pill(ui, "กำลังส่ง", RED_HI, true, true);
            } else {
                pill(ui, "พร้อมส่ง", TEXT2, true, false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if st.active {
                    let mut s = format!("{} เครื่อง", app.engine.send_dests().len());
                    if st.capture_ms > 0.0 {
                        s += &format!(" · จับเสียง {:.0} ms", st.capture_ms);
                    }
                    ui.label(RichText::new(t(s)).size(12.5).color(TEXT2));
                }
            });
        });
        ui.add_space(8.0);
        flip = app.viz_send.show(ui, snap.as_ref(), visual_style(app.s.visual), 120.0).clicked();
        ui.add_space(10.0);
        if let Some(e) = &st.error {
            ui.label(RichText::new(t(format!("ปัญหา: {e}"))).size(12.5).color(ERROR));
        }
        if !st.active {
            start = big_button(ui, &app.g, "เริ่มส่งเสียง", BigKind::Start).clicked();
        } else {
            stop = big_button(ui, &app.g, "หยุดส่ง", BigKind::Stop).clicked();
        }
    });
    if flip {
        app.s.visual = if app.s.visual == Visual::Bars { Visual::Wave } else { Visual::Bars };
        app.save();
    }
    if start {
        start_sending(app);
    }
    if stop {
        app.engine.stop_sending();
    }
}

fn start_sending(app: &mut App) {
    let dests = app.chosen(&app.s.send_to);
    if dests.is_empty() {
        app.toast(t("เลือกเครื่องปลายทางอย่างน้อย 1 เครื่องก่อน"), true);
        return;
    }
    if matches!(&app.s.source, Source::App { key } if key.is_empty()) {
        app.toast(t("เลือกแอปที่จะส่งเสียงก่อน"), true);
        return;
    }
    if matches!(&app.s.source, Source::Input { name } if name.is_empty()) {
        app.toast(t("เลือกอินพุตที่จะส่งก่อน"), true);
        return;
    }
    if let Err(e) = app.engine.start_sending(app.s.source.clone(), dests, app.s.keep_local) {
        app.toast(friendly_error("เริ่มส่งไม่ได้", &e), true);
    }
}

/// Change the source; if sending, carry on with the new one right away.
fn set_source(app: &mut App, source: Source) {
    if source == app.s.source {
        return;
    }
    app.s.source = source;
    app.save();
    if app.engine.sender_stats().active {
        start_sending(app);
    }
}

fn source_card(app: &mut App, ui: &mut Ui) {
    let has_inputs = app.sources.iter().any(|s| s.kind == SourceKind::Input);
    let kinds: Vec<_> = KINDS.iter().filter(|k| k.0 != SourceKind::Input || has_inputs).collect();
    let cur = kind_of(&app.s.source);
    let mut new_source = None;
    let mut refresh = false;
    let mut keep_local_changed = false;
    card(ui, |ui| {
        section_title(ui, "แหล่งเสียง");
        ui.add_space(2.0);
        let labels: Vec<String> = kinds.iter().map(|k| k.1.to_string()).collect();
        let sel = kinds.iter().position(|k| k.0 == cur).unwrap_or(0);
        if let Some(i) = segmented(ui, "source", &labels, sel, true) {
            let k = kinds[i].0;
            if k != cur {
                let first = app.sources.iter().find(|s| s.kind == k).map(|s| s.source.clone());
                new_source = Some(match k {
                    SourceKind::System => Source::System,
                    SourceKind::Tone => Source::Tone,
                    SourceKind::App => first.unwrap_or(Source::App { key: String::new() }),
                    SourceKind::Input => first.unwrap_or(Source::Input { name: String::new() }),
                });
            }
        }
        hint(ui, kinds.iter().find(|k| k.0 == cur).map(|k| k.2).unwrap_or(""));
        if matches!(cur, SourceKind::App | SourceKind::Input) {
            let items: Vec<SourceInfo> = app.sources.iter().filter(|s| s.kind == cur).cloned().collect();
            let current = items.iter().find(|s| s.source == app.s.source).map(|s| short_label(&s.label).to_string());
            let current = current.unwrap_or_else(|| match &app.s.source {
                Source::App { key } if !key.is_empty() => format!("{key} (ไม่ได้เล่นเสียงอยู่)"),
                Source::Input { name } if !name.is_empty() => format!("{name} (ไม่พบ)"),
                _ => "เลือก…".into(),
            });
            ui.horizontal(|ui| {
                let w = ui.available_width() - 46.0;
                egui::ComboBox::from_id_salt("pick-source")
                    .width(w)
                    .height(320.0)
                    .selected_text(RichText::new(t(current)).size(15.0))
                    .show_ui(ui, |ui| {
                        if items.is_empty() {
                            let msg = if cur == SourceKind::App {
                                "ยังไม่มีแอปที่กำลังเล่นเสียง เปิดเสียงในแอปแล้วกดรีเฟรช"
                            } else {
                                "ไม่พบอินพุต"
                            };
                            ui.label(RichText::new(t(msg)).color(TEXT2));
                        }
                        for s in &items {
                            if ui.selectable_label(s.source == app.s.source, t(short_label(&s.label))).clicked() {
                                new_source = Some(s.source.clone());
                            }
                        }
                    });
                refresh = icon_button(ui, 36.0, refresh_icon).on_hover_text(t("รีเฟรชรายการ")).clicked();
            });
            if cur == SourceKind::App {
                ui.add_space(2.0);
                keep_local_changed = toggle_row(
                    ui,
                    "เล่นเสียงแอปนี้ที่เครื่องนี้ด้วย",
                    Some("ปิดไว้ = เสียงแอปดังที่เครื่องปลายทางเท่านั้น"),
                    &mut app.s.keep_local,
                )
                .changed();
            }
        }
    });
    if refresh {
        app.sources = list_sources();
        app.toast(t("อัปเดตรายการแล้ว"), false);
    }
    if keep_local_changed {
        app.save();
        if app.engine.sender_stats().active {
            start_sending(app);
        }
    }
    if let Some(s) = new_source {
        set_source(app, s);
    }
}

fn targets_card(app: &mut App, ui: &mut Ui) {
    let targets = app.targets();
    let mut toggled: Option<String> = None;
    let mut go_settings = false;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            section_title(ui, "ส่งไปที่");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let n = app.s.send_to.len();
                if n > 0 {
                    ui.label(RichText::new(t(format!("เลือก {n} เครื่อง"))).size(12.5).color(RED_HI));
                }
            });
        });
        hint(ui, "เลือกได้หลายเครื่อง ทุกเครื่องได้ยินพร้อมกัน");
        ui.add_space(4.0);
        if targets.is_empty() && app.s.send_to.is_empty() {
            empty_devices(ui, &mut go_settings);
        }
        for tg in &targets {
            let on = app.s.send_to.contains(&tg.key);
            let (detail, color) = if tg.manual {
                ("เพิ่มด้วย IP".to_string(), TEXT2)
            } else if tg.receiving {
                (tg.addr.ip().to_string(), TEXT2)
            } else {
                (format!("{} · ไม่ได้เปิดรับเสียง", tg.addr.ip()), AMBER)
            };
            if device_row(ui, &tg.name, &detail, color, on).clicked() {
                toggled = Some(tg.key.clone());
            }
        }
        // Chosen earlier but not around right now: still listed so it can be unticked.
        for key in app.s.send_to.iter().filter(|k| !targets.iter().any(|t| &t.key == *k)) {
            let ip = key.split(':').next().unwrap_or(key);
            if device_row(ui, ip, "ไม่พบในเครือข่ายตอนนี้", TEXT3, true).clicked() {
                toggled = Some(key.clone());
            }
        }
    });
    if go_settings {
        app.tab = Tab::Settings;
    }
    if let Some(k) = toggled {
        if let Some(i) = app.s.send_to.iter().position(|x| *x == k) {
            app.s.send_to.remove(i);
        } else {
            app.s.send_to.push(k);
        }
        app.save();
    }
}

pub(super) fn empty_devices(ui: &mut Ui, go_settings: &mut bool) {
    egui::Frame::new().fill(SURFACE2).corner_radius(12).inner_margin(16).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(t("ยังไม่พบเครื่องอื่น")).size(15.0).color(TEXT));
            ui.label(
                RichText::new(t("เปิด Stream Sound บนอีกเครื่องที่ต่อ Wi-Fi หรือ LAN เดียวกัน แล้วเครื่องจะขึ้นที่นี่เอง"))
                    .size(12.5)
                    .color(TEXT2),
            );
            ui.add_space(2.0);
            if link_button(ui, "หรือเพิ่มด้วย IP").clicked() {
                *go_settings = true;
            }
        });
    });
    ui.add_space(6.0);
}
