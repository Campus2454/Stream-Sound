//! Desktop window. Simple one-page layout, all text in Thai.

use crate::settings::{self, Settings};
use crate::updater::{self, Status as UpdateStatus};
use eframe::egui::{self, Color32, RichText};
use ssnd_core::proto::DEFAULT_AUDIO_PORT;
use ssnd_core::{list_sources, Engine, EngineConfig, Mode, Peer, Source, SourceInfo, SourceKind};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RED: Color32 = Color32::from_rgb(0xE5, 0x1A, 0x2E);
const RED_DIM: Color32 = Color32::from_rgb(0x7A, 0x10, 0x1A);
const PANEL: Color32 = Color32::from_rgb(0x16, 0x16, 0x18);
const BG: Color32 = Color32::from_rgb(0x0C, 0x0C, 0x0D);

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Stream Sound")
            .with_inner_size([540.0, 760.0])
            .with_min_inner_size([380.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native("Stream Sound", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// egui draws text without OpenType shaping, so Thai tone marks would sit on
/// top of upper vowels. Swap in the pre-positioned mark glyphs (the classic
/// Windows private-use variants, which Loma carries) to place them correctly.
fn t(s: impl AsRef<str>) -> String {
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

fn setup_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "thai".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!("../assets/Loma.ttf"))),
    );
    // First in line: Loma covers Latin too, and its Thai mark glyphs must win
    // over the icon font, which also uses the private-use area.
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&fam) {
            list.insert(0, "thai".to_owned());
        }
    }
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = Color32::from_rgb(0x1E, 0x1E, 0x21);
    v.selection.bg_fill = RED;
    v.hyperlink_color = RED;
    v.widgets.hovered.bg_stroke.color = RED;
    v.widgets.active.bg_fill = RED_DIM;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        for (_, f) in s.text_styles.iter_mut() {
            f.size *= 1.12;
        }
    });
}

struct App {
    engine: Engine,
    sources: Vec<SourceInfo>,
    source_idx: usize,
    keep_local: bool,
    settings: Settings,
    ips: Vec<Ipv4Addr>,
    last_ip_poll: Instant,
    send_to: BTreeSet<String>,
    forward_to: BTreeSet<String>,
    manual: Vec<String>,
    manual_input: String,
    peers: Vec<Peer>,
    last_peer_poll: Instant,
    message: Option<(String, Instant)>,
    update: Arc<parking_lot::Mutex<UpdateStatus>>,
    last_update_check: Instant,
    /// After an update restart the old copy may still hold the port briefly.
    receive_retry_until: Option<Instant>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        setup_style(&cc.egui_ctx);
        let settings = settings::load();
        let engine = Engine::new(EngineConfig { mode: settings.mode, ..EngineConfig::default() });
        engine.set_volume(settings.volume);
        let mut app = App {
            ips: engine.local_ips(),
            last_ip_poll: Instant::now(),
            engine,
            sources: list_sources(),
            source_idx: 0,
            keep_local: false,
            settings,
            send_to: BTreeSet::new(),
            forward_to: BTreeSet::new(),
            manual: Vec::new(),
            manual_input: String::new(),
            peers: Vec::new(),
            last_peer_poll: Instant::now() - Duration::from_secs(5),
            message: None,
            update: Arc::new(parking_lot::Mutex::new(UpdateStatus::Idle)),
            last_update_check: Instant::now(),
            receive_retry_until: None,
        };
        // Receiving is on by default so other devices can send here right away.
        if app.engine.start_receiving().is_err() {
            app.receive_retry_until = Some(Instant::now() + Duration::from_secs(5));
        }
        updater::cleanup();
        updater::check_and_download_in_background(app.update.clone());
        app
    }

    fn flash(&mut self, msg: String) {
        self.message = Some((msg, Instant::now()));
    }

    /// Every destination the user can tick: discovered devices plus typed-in addresses.
    fn targets(&self) -> Vec<(String, String, SocketAddr, bool)> {
        let mut v: Vec<(String, String, SocketAddr, bool)> =
            self.peers.iter().map(|p| (p.id.clone(), p.name.clone(), p.addr(), p.receiving)).collect();
        for m in &self.manual {
            if let Some(a) = resolve(m) {
                v.push((format!("ip:{m}"), m.clone(), a, true));
            }
        }
        v
    }

    fn selected(&self, set: &BTreeSet<String>) -> Vec<SocketAddr> {
        self.targets().into_iter().filter(|t| set.contains(&t.0)).map(|t| t.2).collect()
    }

    fn sync_engine(&mut self) {
        let send = self.selected(&self.send_to);
        if send != self.engine.send_dests() {
            self.engine.set_send_dests(send);
        }
        let fw = self.selected(&self.forward_to);
        if fw != self.engine.forward_targets() {
            self.engine.set_forward(fw);
        }
    }
}

fn mode_label(m: Mode) -> &'static str {
    match m {
        Mode::Game => "เกม (หน่วงต่ำสุด)",
        Mode::Balanced => "สมดุล",
        Mode::Music => "ฟังเพลง (เสถียรสุด)",
    }
}

fn mode_hint(m: Mode) -> &'static str {
    match m {
        Mode::Game => "เสียงตรงกับภาพที่สุด เสียงที่มาช้าจะถูกทิ้ง ถ้า Wi-Fi สะดุดอาจได้ยินเสียงแตกสั้น ๆ",
        Mode::Balanced => "หน่วงต่ำและไม่สะดุด ใช้ได้ทั่วไป (แนะนำ)",
        Mode::Music => "หน่วงมากขึ้น แต่ทน Wi-Fi ที่ไม่เสถียรได้ดีที่สุด",
    }
}

/// "หน่วงรวม ~35 ms (จับเสียง 10 + บัฟเฟอร์ 15 + ลำโพง 10)"
fn delay_line(s: &ssnd_core::StreamStats, out_ms: f64) -> String {
    let mut parts = Vec::new();
    if s.capture_ms > 0.0 {
        parts.push(format!("จับเสียง {:.0}", s.capture_ms));
    }
    parts.push(format!("บัฟเฟอร์ {:.0}", s.buffer_ms));
    if out_ms > 0.0 {
        parts.push(format!("ลำโพง {:.0}", out_ms));
    }
    format!("หน่วงรวม ~{:.0} ms ({})", s.capture_ms + s.buffer_ms + out_ms, parts.join(" + "))
}

fn resolve(s: &str) -> Option<SocketAddr> {
    let s = s.trim();
    let full = if s.contains(':') { s.to_string() } else { format!("{s}:{DEFAULT_AUDIO_PORT}") };
    full.to_socket_addrs().ok()?.find(|a| a.is_ipv4())
}

fn card(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL)
        .corner_radius(10.0)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x2A, 0x2A, 0x2E)))
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(20.0).strong().color(Color32::WHITE));
            ui.add_space(4.0);
            body(ui);
        });
    ui.add_space(10.0);
}

fn level_bar(ui: &mut egui::Ui, level: f32) {
    ui.add(egui::ProgressBar::new(level.clamp(0.0, 1.0)).desired_height(6.0).fill(RED));
}

fn big_button(ui: &mut egui::Ui, text: &str, active: bool) -> bool {
    let fill = if active { RED_DIM } else { RED };
    ui.add_sized(
        [ui.available_width(), 40.0],
        egui::Button::new(RichText::new(text).size(18.0).color(Color32::WHITE)).fill(fill),
    )
    .clicked()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_peer_poll.elapsed() > Duration::from_millis(500) {
            self.peers = self.engine.peers();
            self.last_peer_poll = Instant::now();
        }
        if self.last_ip_poll.elapsed() > Duration::from_secs(5) {
            self.ips = self.engine.local_ips();
            self.last_ip_poll = Instant::now();
        }
        self.sync_engine();
        if let Some(until) = self.receive_retry_until {
            match self.engine.start_receiving() {
                Ok(()) => self.receive_retry_until = None,
                Err(e) if Instant::now() > until => {
                    self.receive_retry_until = None;
                    self.flash(t(format!("เปิดรับเสียงไม่ได้: {e}")));
                }
                Err(_) => {}
            }
        }
        // Look for a new version every 6 hours while running.
        if self.last_update_check.elapsed() > Duration::from_secs(6 * 3600) {
            self.last_update_check = Instant::now();
            updater::check_and_download_in_background(self.update.clone());
        }
        ctx.request_repaint_after(Duration::from_millis(100));

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Stream Sound").size(26.0).strong().color(RED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ip = self.ips.first().map(|i| i.to_string()).unwrap_or_else(|| t("ไม่พบ IP"));
                        let r = ui.label(RichText::new(t(format!("เครื่องนี้: {} ({ip})", self.engine.name()))).weak());
                        if self.ips.len() > 1 {
                            let all: Vec<String> = self.ips.iter().map(|i| i.to_string()).collect();
                            r.on_hover_text(all.join("\n"));
                        }
                    });
                });
                if let Some((m, t)) = &self.message {
                    if t.elapsed() < Duration::from_secs(8) {
                        ui.colored_label(Color32::from_rgb(0xFF, 0x80, 0x80), m);
                    }
                }
                self.update_banner(ui);
                ui.add_space(6.0);
                self.mode_card(ui);
                self.send_card(ui);
                self.receive_card(ui);
                self.devices_card(ui);
                self.version_footer(ui);
            });
        });
    }
}

impl App {
    fn update_banner(&mut self, ui: &mut egui::Ui) {
        let status = self.update.lock().clone();
        let UpdateStatus::Ready { release, file } = status else { return };
        egui::Frame::new().fill(RED_DIM).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(t(format!("มีเวอร์ชันใหม่พร้อมแล้ว (build {})", release.build))).color(Color32::WHITE).strong());
            let busy = self.engine.sender_stats().active || !self.engine.streams().is_empty();
            if busy {
                ui.label(RichText::new(t("กำลังสตรีมอยู่ กดอัปเดตแล้วเสียงจะหยุดสักครู่")).color(Color32::WHITE).size(13.0));
            }
            if ui.button(t("รีสตาร์ตเพื่ออัปเดต")).clicked() {
                // Release capture sinks and the port before handing over.
                self.engine.stop_sending();
                self.engine.stop_receiving();
                if let Err(e) = updater::install_and_restart(&file) {
                    *self.update.lock() = UpdateStatus::Failed(format!("{e:#}"));
                    let _ = self.engine.start_receiving();
                }
            }
        });
        ui.add_space(6.0);
    }

    fn version_footer(&mut self, ui: &mut egui::Ui) {
        let status = self.update.lock().clone();
        ui.horizontal(|ui| {
            let build = updater::current_build();
            let ver = if build == 0 { "dev".to_string() } else { format!("build {build}") };
            ui.label(RichText::new(t(format!("เวอร์ชัน {ver}"))).weak().size(13.0));
            let text = match &status {
                UpdateStatus::Idle => String::new(),
                UpdateStatus::Checking => t("กำลังตรวจสอบอัปเดต…"),
                UpdateStatus::UpToDate => t("เป็นเวอร์ชันล่าสุดแล้ว"),
                UpdateStatus::NoRelease => t("ยังไม่มีเวอร์ชันที่เผยแพร่บน GitHub"),
                UpdateStatus::Downloading { release, percent } => {
                    t(format!("กำลังดาวน์โหลด build {} ({percent}%)", release.build))
                }
                UpdateStatus::Ready { .. } => t("อัปเดตพร้อมติดตั้ง"),
                UpdateStatus::Failed(e) => t(format!("ตรวจสอบอัปเดตไม่ได้: {e}")),
            };
            ui.label(RichText::new(text).weak().size(13.0));
            let idle = matches!(
                status,
                UpdateStatus::Idle | UpdateStatus::UpToDate | UpdateStatus::NoRelease | UpdateStatus::Failed(_)
            );
            if idle && ui.small_button(t("ตรวจสอบอัปเดต")).clicked() {
                self.last_update_check = Instant::now();
                *self.update.lock() = UpdateStatus::Idle;
                updater::check_and_download_in_background(self.update.clone());
            }
        });
    }

    fn mode_card(&mut self, ui: &mut egui::Ui) {
        card(ui, &t("โหมด"), |ui| {
            let before = self.settings.mode;
            ui.horizontal_wrapped(|ui| {
                for m in Mode::ALL {
                    ui.selectable_value(&mut self.settings.mode, m, t(mode_label(m)));
                }
            });
            ui.label(RichText::new(t(mode_hint(self.settings.mode))).weak().size(13.0));
            if self.settings.mode != before {
                self.engine.set_mode(self.settings.mode);
                settings::save(&self.settings);
            }
        });
    }

    fn destination_checks(&mut self, ui: &mut egui::Ui, forward: bool) {
        let targets = self.targets();
        if targets.is_empty() {
            ui.label(RichText::new(t("ยังไม่พบอุปกรณ์อื่นในเครือข่าย เปิดแอปนี้บนอีกเครื่อง หรือเพิ่ม IP เอง")).weak());
        }
        for (id, name, addr, receiving) in targets {
            let set = if forward { &mut self.forward_to } else { &mut self.send_to };
            let mut on = set.contains(&id);
            let status = if receiving { String::new() } else { t("  (ไม่ได้เปิดรับ)") };
            if ui.checkbox(&mut on, format!("{name}  {}{status}", addr.ip())).changed() {
                if on {
                    set.insert(id);
                } else {
                    set.remove(&id);
                }
            }
        }
    }

    fn send_card(&mut self, ui: &mut egui::Ui) {
        card(ui, &t("ส่งเสียง"), |ui| {
            let sending = self.engine.sender_stats().active;
            ui.horizontal(|ui| {
                ui.label(t("แหล่งเสียง"));
                let current = self.sources.get(self.source_idx).map(|s| s.label.clone()).unwrap_or_default();
                ui.add_enabled_ui(!sending, |ui| {
                    egui::ComboBox::from_id_salt("source")
                        .selected_text(t(current))
                        .width(ui.available_width() - 70.0)
                        .show_ui(ui, |ui| {
                            for (i, s) in self.sources.iter().enumerate() {
                                ui.selectable_value(&mut self.source_idx, i, t(&s.label));
                            }
                        });
                    if ui.button(t("รีเฟรช")).clicked() {
                        let prev = self.sources.get(self.source_idx).map(|s| s.source.clone());
                        self.sources = list_sources();
                        self.source_idx = prev
                            .and_then(|p| self.sources.iter().position(|s| s.source == p))
                            .unwrap_or(0);
                    }
                });
            });
            let is_app = self.sources.get(self.source_idx).map(|s| s.kind == SourceKind::App).unwrap_or(false);
            if is_app {
                ui.add_enabled(!sending, egui::Checkbox::new(&mut self.keep_local, t("เล่นเสียงแอปนี้ที่เครื่องนี้ด้วย")));
            }
            ui.label(RichText::new(t("ส่งไปที่ (เลือกได้หลายเครื่อง)")).strong());
            self.destination_checks(ui, false);

            if !sending {
                if big_button(ui, &t("เริ่มส่งเสียง"), false) {
                    let dests = self.selected(&self.send_to);
                    if dests.is_empty() {
                        self.flash(t("เลือกเครื่องปลายทางอย่างน้อย 1 เครื่องก่อน").into());
                    } else {
                        let src = self.sources.get(self.source_idx).map(|s| s.source.clone()).unwrap_or(Source::System);
                        if let Err(e) = self.engine.start_sending(src, dests, self.keep_local) {
                            self.flash(t(format!("เริ่มส่งไม่ได้: {e}")));
                        }
                    }
                }
            } else {
                let st = self.engine.sender_stats();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("● กำลังส่ง")).color(RED));
                    ui.label(RichText::new(t(format!("{} แพ็กเก็ต", st.packets))).weak());
                    if st.capture_ms > 0.0 {
                        ui.label(RichText::new(t(format!("· จับเสียงหน่วง {:.0} ms", st.capture_ms))).weak());
                    }
                });
                level_bar(ui, st.level);
                if let Some(e) = st.error {
                    ui.colored_label(Color32::from_rgb(0xFF, 0x80, 0x80), t(format!("ปัญหา: {e}")));
                }
                if big_button(ui, &t("หยุดส่ง"), true) {
                    self.engine.stop_sending();
                }
            }
        });
    }

    fn receive_card(&mut self, ui: &mut egui::Ui) {
        card(ui, &t("รับเสียง"), |ui| {
            let mut on = self.engine.is_receiving();
            if ui.checkbox(&mut on, t("เปิดรับเสียงจากเครื่องอื่น")).changed() {
                if on {
                    if let Err(e) = self.engine.start_receiving() {
                        self.flash(t(format!("เปิดรับเสียงไม่ได้: {e}")));
                    }
                } else {
                    self.engine.stop_receiving();
                }
            }
            let mut play = self.engine.play_local();
            if ui.checkbox(&mut play, t("เล่นเสียงที่เครื่องนี้")).changed() {
                self.engine.set_play_local(play);
            }
            let vol = ui.add(egui::Slider::new(&mut self.settings.volume, 0.0..=1.5).text(t("ระดับเสียง")).show_value(false));
            if vol.changed() {
                self.engine.set_volume(self.settings.volume);
            }
            if vol.drag_stopped() || (vol.changed() && !vol.dragged()) {
                settings::save(&self.settings);
            }
            if let Some(e) = self.engine.receiver_error() {
                ui.colored_label(Color32::from_rgb(0xFF, 0x80, 0x80), t(format!("ลำโพง: {e}")));
            }

            let streams = self.engine.streams();
            if streams.is_empty() && self.engine.is_receiving() {
                ui.label(RichText::new(t("รอเสียงจากเครื่องอื่น…")).weak());
            }
            let out_ms = self.engine.output_latency_ms();
            for s in &streams {
                ui.label(RichText::new(t(&s.name)).strong());
                ui.label(RichText::new(t(delay_line(s, out_ms))).size(14.0));
                ui.label(
                    RichText::new(t(format!(
                        "หาย {} · มาช้า {} · สะดุด {} · {} kHz",
                        s.lost,
                        s.late,
                        s.underruns,
                        s.sample_rate as f32 / 1000.0
                    )))
                    .weak()
                    .size(13.0),
                );
                level_bar(ui, s.level);
            }
            if !streams.is_empty() {
                ui.label(RichText::new(t("ไม่รวมเวลาเดินทางในเครือข่าย (สาย LAN ~1 ms, Wi-Fi ~2–10 ms)")).weak().size(12.0));
            }

            ui.add_space(4.0);
            egui::CollapsingHeader::new(t("ส่งต่อเสียงที่รับได้ไปเครื่องอื่น (ต่อเป็นทอด)")).show(ui, |ui| {
                self.destination_checks(ui, true);
                let n = self.engine.forwarded_packets();
                if n > 0 {
                    ui.label(RichText::new(t(format!("ส่งต่อแล้ว {n} แพ็กเก็ต"))).weak());
                }
            });
        });
    }

    fn devices_card(&mut self, ui: &mut egui::Ui) {
        card(ui, &t("อุปกรณ์"), |ui| {
            ui.label(RichText::new(t("เครื่องที่เปิดแอปนี้ในวง LAN เดียวกันจะขึ้นเองอัตโนมัติ")).weak().size(13.0));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.manual_input).hint_text(t("IP เช่น 192.168.1.20")).desired_width(200.0));
                if ui.button(t("เพิ่ม")).clicked() {
                    let s = self.manual_input.trim().to_string();
                    if resolve(&s).is_some() {
                        if !self.manual.contains(&s) {
                            self.manual.push(s);
                        }
                        self.manual_input.clear();
                    } else {
                        self.flash(t("IP ไม่ถูกต้อง").into());
                    }
                }
            });
            let mut remove = None;
            for (i, m) in self.manual.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(m);
                    if ui.small_button(t("ลบ")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                let m = self.manual.remove(i);
                self.send_to.remove(&format!("ip:{m}"));
                self.forward_to.remove(&format!("ip:{m}"));
            }
        });
    }
}
