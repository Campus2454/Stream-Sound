//! Desktop window: header, three tabs (send / receive / settings), update
//! banner and toasts. All text is Thai. Layout follows docs/DESIGN.md.

mod receive;
mod send;
mod settings_tab;
mod theme;
mod widgets;

use crate::settings::{self, Settings, Visual};
use crate::updater::{self, Status as UpdateStatus};
use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, RichText, Sense};
use ssnd_core::proto::DEFAULT_AUDIO_PORT;
use ssnd_core::{list_sources, Engine, EngineConfig, Peer, SourceInfo};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};
use theme::*;
use widgets::*;

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Stream Sound")
            .with_app_id("stream-sound")
            .with_icon(theme::app_icon())
            .with_inner_size([500.0, 840.0])
            .with_min_inner_size([400.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("Stream Sound", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
        .map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Send,
    Receive,
    Settings,
}

/// A device the user can send or forward to.
struct Target {
    /// "ip:port", also the key the choice is remembered by.
    key: String,
    name: String,
    addr: SocketAddr,
    /// Known to be listening (always true for typed-in addresses).
    receiving: bool,
    manual: bool,
}

pub struct App {
    engine: Engine,
    s: Settings,
    g: Gradients,
    tab: Tab,
    sources: Vec<SourceInfo>,
    ips: Vec<Ipv4Addr>,
    peers: Vec<Peer>,
    toast: Option<(String, Instant, bool)>,
    update: Arc<parking_lot::Mutex<UpdateStatus>>,
    last_update_check: Instant,
    last_peer_poll: Instant,
    last_ip_poll: Instant,
    /// After an update restart the old copy may still hold the port briefly.
    receive_retry_until: Option<Instant>,
    viz_send: Viz,
    viz_recv: Viz,
    tube_level: f32,
    manual_input: String,
    name_input: String,
    expanded: Option<u32>,
    /// Per stream: underrun count last seen and when it last went up.
    stutter: HashMap<u32, (u64, Option<Instant>)>,
    relay_open: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> App {
        let g = theme::setup(&cc.egui_ctx);
        let s = settings::load();
        let mut cfg = EngineConfig { mode: s.mode, ..EngineConfig::default() };
        if let Some(n) = &s.name {
            cfg.name = n.clone();
        }
        let engine = Engine::new(cfg);
        engine.set_volume(if s.muted { 0.0 } else { s.volume });
        engine.set_play_local(s.play_local);
        let mut app = App {
            ips: engine.local_ips(),
            name_input: engine.name(),
            engine,
            s,
            g,
            tab: Tab::Send,
            sources: list_sources(),
            peers: Vec::new(),
            toast: None,
            update: Arc::new(parking_lot::Mutex::new(UpdateStatus::Idle)),
            last_update_check: Instant::now(),
            last_peer_poll: Instant::now() - Duration::from_secs(5),
            last_ip_poll: Instant::now(),
            receive_retry_until: None,
            viz_send: Viz::default(),
            viz_recv: Viz::default(),
            tube_level: 0.0,
            manual_input: String::new(),
            expanded: None,
            stutter: HashMap::new(),
            relay_open: false,
        };
        if app.s.auto_receive {
            // Receive right away so other devices can send here.
            if app.engine.start_receiving().is_err() {
                app.receive_retry_until = Some(Instant::now() + Duration::from_secs(5));
            }
        }
        updater::cleanup();
        updater::check_and_download_in_background(app.update.clone());
        app
    }

    fn save(&self) {
        settings::save(&self.s);
    }

    fn toast(&mut self, msg: impl Into<String>, error: bool) {
        self.toast = Some((msg.into(), Instant::now(), error));
    }

    /// Discovered devices plus typed-in addresses, without duplicates.
    fn targets(&self) -> Vec<Target> {
        let mut v: Vec<Target> = self
            .peers
            .iter()
            .map(|p| Target { key: p.addr().to_string(), name: p.name.clone(), addr: p.addr(), receiving: p.receiving, manual: false })
            .collect();
        for m in &self.s.manual {
            if let Some(a) = resolve(m) {
                if !v.iter().any(|t| t.addr == a) {
                    v.push(Target { key: a.to_string(), name: m.clone(), addr: a, receiving: true, manual: true });
                }
            }
        }
        v
    }

    fn chosen(&self, keys: &[String]) -> Vec<SocketAddr> {
        keys.iter().filter_map(|k| k.parse().ok()).collect()
    }

    fn sync_engine(&mut self) {
        let send = self.chosen(&self.s.send_to);
        if send != self.engine.send_dests() {
            self.engine.set_send_dests(send);
        }
        let fw = self.chosen(&self.s.forward_to);
        if fw != self.engine.forward_targets() {
            self.engine.set_forward(fw);
        }
    }

    fn start_receiving(&mut self) {
        if let Err(e) = self.engine.start_receiving() {
            self.toast(friendly_error("เปิดรับเสียงไม่ได้", &e), true);
        }
    }

    fn poll(&mut self) {
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
                    self.toast(friendly_error("เปิดรับเสียงไม่ได้", &e), true);
                }
                Err(_) => {}
            }
        }
        // Look for a new version every 6 hours while running.
        if self.last_update_check.elapsed() > Duration::from_secs(6 * 3600) {
            self.last_update_check = Instant::now();
            updater::check_and_download_in_background(self.update.clone());
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            logo(ui, &self.g, 30.0);
            ui.add_space(2.0);
            ui.label(RichText::new("Stream Sound").size(20.0).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, dot) = match self.ips.first() {
                    Some(ip) => (format!("{} · {ip}", self.engine.name()), GREEN),
                    None => (format!("{} · ไม่ได้ต่อเครือข่าย", self.engine.name()), AMBER),
                };
                let galley = ui.painter().layout_no_wrap(t(&text), font(13.0), TEXT2);
                let size = vec2(galley.size().x + 34.0, 30.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let hover = resp.hovered();
                ui.painter().rect_filled(rect, 15, if hover { OUTLINE } else { SURFACE2 });
                ui.painter().circle_filled(pos2(rect.left() + 14.0, rect.center().y), 4.0, dot);
                ui.painter().galley(pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, TEXT2);
                let all: Vec<String> = self.ips.iter().map(|i| i.to_string()).collect();
                let tip = if all.is_empty() { t("ยังไม่มี IP") } else { t(format!("คลิกเพื่อคัดลอก IP\n{}", all.join("\n"))) };
                if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    if let Some(ip) = self.ips.first() {
                        ui.ctx().copy_text(ip.to_string());
                        self.toast(t(format!("คัดลอก {ip} แล้ว")), false);
                    }
                }
            });
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(rect, 14, SURFACE);
        let tabs = [(Tab::Send, "ส่งเสียง"), (Tab::Receive, "รับเสียง"), (Tab::Settings, "ตั้งค่า")];
        let w = rect.width() / 3.0;
        let idx = tabs.iter().position(|(tb, _)| *tb == self.tab).unwrap_or(0) as f32;
        let pos = ui.ctx().animate_value_with_time(egui::Id::new("tab-pos"), idx, 0.18);
        let sel = Rect::from_min_size(pos2(rect.left() + pos * w, rect.top()), vec2(w, rect.height())).shrink(4.0);
        p.rect_filled(sel, 11, RED);
        let sending = self.engine.sender_stats().active;
        let receiving_audio = !self.engine.streams().is_empty();
        for (i, (tab, label)) in tabs.iter().enumerate() {
            let r = Rect::from_min_size(pos2(rect.left() + i as f32 * w, rect.top()), vec2(w, rect.height()));
            let resp = ui.interact(r, egui::Id::new("tab").with(i), Sense::click());
            let on = self.tab == *tab;
            let color = if on || resp.hovered() { TEXT } else { TEXT2 };
            let galley = p.layout_no_wrap(t(label), font(15.0), color);
            let total = 20.0 + 8.0 + galley.size().x;
            let x0 = r.center().x - total / 2.0;
            let icon = Rect::from_center_size(pos2(x0 + 10.0, r.center().y), vec2(20.0, 20.0));
            match tab {
                Tab::Send => send_icon(ui, icon, color),
                Tab::Receive => receive_icon(ui, icon, color),
                Tab::Settings => settings_icon(ui, icon, color),
            }
            p.galley(pos2(x0 + 28.0, r.center().y - galley.size().y / 2.0), galley, color);
            let dot = match tab {
                Tab::Send if sending => Some(if on { Color32::WHITE } else { RED }),
                Tab::Receive if receiving_audio => Some(GREEN),
                _ => None,
            };
            if let Some(c) = dot {
                p.circle_filled(pos2(x0 + total + 7.0, r.center().y - 6.0), 3.5, c);
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.tab = *tab;
            }
        }
    }

    fn update_banner(&mut self, ui: &mut egui::Ui) {
        let status = self.update.lock().clone();
        let UpdateStatus::Ready { release, file } = status else { return };
        egui::Frame::new()
            .fill(RED_DEEP)
            .stroke(egui::Stroke::new(1.0, RED))
            .corner_radius(16)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 110.0);
                        ui.label(RichText::new(t(format!("มีเวอร์ชันใหม่ (build {})", release.build))).size(15.0).color(TEXT));
                        let busy = self.engine.sender_stats().active || !self.engine.streams().is_empty();
                        let note = if busy { "เสียงจะหยุดสักครู่ระหว่างอัปเดต" } else { "ใช้เวลาไม่กี่วินาที" };
                        ui.label(RichText::new(t(note)).size(12.5).color(Color32::from_white_alpha(190)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let b = egui::Button::new(RichText::new(t("อัปเดตเลย")).color(RED_DEEP).size(14.5))
                            .fill(Color32::WHITE)
                            .corner_radius(10);
                        if ui.add(b).clicked() {
                            // Release capture sinks and the port before handing over.
                            self.engine.stop_sending();
                            self.engine.stop_receiving();
                            if let Err(e) = updater::install_and_restart(&file) {
                                *self.update.lock() = UpdateStatus::Failed(format!("{e:#}"));
                                self.start_receiving();
                            }
                        }
                    });
                });
            });
        ui.add_space(12.0);
    }

    fn show_toast(&mut self, ctx: &egui::Context) {
        let Some((msg, at, error)) = self.toast.clone() else { return };
        let age = at.elapsed().as_secs_f32();
        let life = if error { 6.0 } else { 3.0 };
        if age > life {
            self.toast = None;
            return;
        }
        let alpha = ((life - age) / 0.3).min(age / 0.15).clamp(0.0, 1.0);
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -22.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                let fill = if error { Color32::from_rgb(0x3A, 0x12, 0x18) } else { Color32::from_rgb(0x26, 0x26, 0x2D) };
                let r = egui::Frame::new()
                    .fill(fill.gamma_multiply(alpha))
                    .stroke(egui::Stroke::new(1.0, if error { ERROR } else { OUTLINE }.gamma_multiply(alpha)))
                    .corner_radius(20)
                    .inner_margin(egui::Margin::symmetric(18, 10))
                    .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha((120.0 * alpha) as u8) })
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.label(RichText::new(&msg).size(14.0).color(TEXT.gamma_multiply(alpha)));
                    });
                if r.response.interact(Sense::click()).clicked() {
                    self.toast = None;
                }
            });
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let panel = egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 16, right: 16, top: 14, bottom: 10 });
        egui::TopBottomPanel::top("top").frame(panel).show_separator_line(false).show(ctx, |ui| {
            let w = ui.available_width().min(620.0);
            ui.vertical_centered(|ui| {
                ui.set_width(w);
                self.header(ui);
                ui.add_space(12.0);
                self.tab_bar(ui);
            });
        });
        let body = egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 16, right: 16, top: 4, bottom: 8 });
        egui::CentralPanel::default().frame(body).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let w = ui.available_width().min(620.0);
                ui.vertical_centered(|ui| {
                    ui.set_width(w);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        self.update_banner(ui);
                        match self.tab {
                            Tab::Send => send::show(self, ui),
                            Tab::Receive => receive::show(self, ui),
                            Tab::Settings => settings_tab::show(self, ui),
                        }
                        ui.add_space(24.0);
                    });
                });
            });
        });
        self.show_toast(ctx);
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

fn resolve(s: &str) -> Option<SocketAddr> {
    let s = s.trim();
    let full = if s.contains(':') { s.to_string() } else { format!("{s}:{DEFAULT_AUDIO_PORT}") };
    full.to_socket_addrs().ok()?.find(|a| a.is_ipv4())
}

/// Turn common OS errors into something a person can act on.
fn friendly_error(what: &str, e: &anyhow::Error) -> String {
    let raw = format!("{e:#}");
    let l = raw.to_lowercase();
    let why = if l.contains("address already in use") || l.contains("10048") || l.contains("only one usage") {
        "พอร์ต 47800 ถูกใช้อยู่ (อาจเปิดแอปนี้ซ้อนอยู่แล้ว)".to_string()
    } else if l.contains("permission") || l.contains("access is denied") || l.contains("10013") {
        "ระบบไม่อนุญาต (ลองอนุญาตแอปใน Firewall)".to_string()
    } else if l.contains("no such device") || l.contains("device not found") || l.contains("no default") {
        "ไม่พบอุปกรณ์เสียง".to_string()
    } else {
        raw
    };
    t(format!("{what}: {why}"))
}

fn visual_style(v: Visual) -> VizStyle {
    match v {
        Visual::Bars => VizStyle::Bars,
        Visual::Wave => VizStyle::Wave,
    }
}
