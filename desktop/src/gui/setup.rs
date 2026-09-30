//! The Linux installer's window, in the app's own look: install (with the
//! folder and shortcuts to choose), update (progress only, started by the
//! app's updater) and uninstall. The work itself is in setup::linux.

use super::theme::*;
use super::widgets::*;
use crate::setup::linux as inst;
use eframe::egui::{self, vec2, Align, Color32, Layout, RichText, Stroke};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub enum Mode {
    Install,
    /// Replace this program (the copy that started the update).
    Update(PathBuf),
    Uninstall,
}

pub fn run(mode: Mode) -> anyhow::Result<()> {
    let (title, size) = match mode {
        Mode::Install => ("ติดตั้ง Stream Sound", [460.0, 660.0]),
        Mode::Update(_) => ("อัปเดต Stream Sound", [420.0, 230.0]),
        Mode::Uninstall => ("ถอนการติดตั้ง Stream Sound", [440.0, 470.0]),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_app_id("stream-sound")
            .with_icon(app_icon())
            .with_inner_size(size)
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(title, options, Box::new(move |cc| Ok(Box::new(Setup::new(cc, mode)))))
        .map_err(|e| anyhow::anyhow!("{e}"))
}

enum Stage {
    Ask,
    Working,
    Done,
    Failed(String),
}

/// Shared with the thread doing the work.
#[derive(Default)]
struct Progress {
    value: f32,
    text: String,
    /// Set when the work ends.
    result: Option<Result<(), String>>,
}

/// The work, given a way to report progress (0..1 and what is happening).
type Job = Box<dyn FnOnce(&dyn Fn(f32, &str)) -> anyhow::Result<()> + Send>;

struct Setup {
    g: Gradients,
    mode: Mode,
    stage: Stage,
    progress: Arc<parking_lot::Mutex<Progress>>,
    dir: String,
    desktop_shortcut: bool,
    menu: bool,
    launch: bool,
    /// Already installed there.
    existing: Option<PathBuf>,
    picker: Option<&'static str>,
    picking: Option<Receiver<Option<PathBuf>>>,
    remove_settings: bool,
    /// When to close the window by itself.
    close_at: Option<Instant>,
}

impl Setup {
    fn new(cc: &eframe::CreationContext<'_>, mode: Mode) -> Setup {
        let g = super::theme::setup(&cc.egui_ctx);
        let r = inst::load();
        let existing = r.exe.clone().filter(|e| e.exists());
        let dir = existing.as_ref().and_then(|e| e.parent().map(PathBuf::from)).unwrap_or_else(inst::default_dir);
        let fresh = existing.is_none();
        let mut s = Setup {
            g,
            mode,
            stage: Stage::Ask,
            progress: Arc::default(),
            dir: dir.to_string_lossy().into_owned(),
            desktop_shortcut: fresh || r.desktop_shortcut,
            menu: fresh || r.menu,
            launch: true,
            existing,
            picker: inst::folder_picker(),
            picking: None,
            remove_settings: false,
            close_at: None,
        };
        if let Mode::Update(_) = s.mode {
            s.start(&cc.egui_ctx);
        }
        s
    }

    /// Do the work for this mode in the background.
    fn start(&mut self, ctx: &egui::Context) {
        self.stage = Stage::Working;
        *self.progress.lock() = Progress::default();
        let progress = self.progress.clone();
        let ctx = ctx.clone();
        let job: Job = match &self.mode {
            Mode::Install => {
                let o = inst::Options { dir: PathBuf::from(self.dir.trim()), desktop_shortcut: self.desktop_shortcut, menu: self.menu };
                let launch = self.launch;
                Box::new(move |step| {
                    let exe = inst::install(&o, step)?;
                    if launch {
                        inst::launch(&exe, &[])?;
                    }
                    Ok(())
                })
            }
            Mode::Update(target) => {
                let target = target.clone();
                Box::new(move |step| {
                    inst::update(&target, step)?;
                    inst::launch(&target, &[crate::os::AFTER_UPDATE_FLAG])
                })
            }
            Mode::Uninstall => {
                let settings = self.remove_settings;
                Box::new(move |step| inst::uninstall(settings, step))
            }
        };
        let _ = std::thread::Builder::new().name("ssnd-setup".into()).spawn(move || {
            let step = |v: f32, text: &str| {
                let mut p = progress.lock();
                p.value = v;
                p.text = text.to_string();
                ctx.request_repaint();
            };
            let result = job(&step).map_err(|e| format!("{e:#}"));
            progress.lock().result = Some(result);
            ctx.request_repaint();
        });
    }

    fn header(&self, ui: &mut egui::Ui, title: &str, sub: &str) {
        ui.horizontal(|ui| {
            logo(ui, &self.g, 44.0);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(RichText::new(t(title)).size(20.0).color(TEXT));
                ui.label(RichText::new(t(sub)).size(13.0).color(TEXT2));
            });
        });
        ui.add_space(16.0);
    }

    fn version() -> String {
        match crate::updater::current() {
            Some(v) => format!("เวอร์ชัน {v}"),
            None => "เวอร์ชันทดสอบ".into(),
        }
    }

    fn install_page(&mut self, ui: &mut egui::Ui) {
        let updating = self.existing.as_ref().is_some_and(|e| e.parent() == Some(PathBuf::from(self.dir.trim()).as_path()));
        self.header(ui, "ติดตั้ง Stream Sound", &Self::version());
        if let Some(e) = &self.existing {
            hint(ui, &format!("มี Stream Sound ติดตั้งอยู่แล้วที่ {}", e.display()));
            ui.add_space(8.0);
        }
        card(ui, |ui| {
            section_title(ui, "ติดตั้งที่");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let picker_w = if self.picker.is_some() { 76.0 } else { 0.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut self.dir)
                        .desired_width(ui.available_width() - picker_w)
                        .margin(vec2(12.0, 9.0))
                        .font(font(14.0)),
                );
                if let Some(tool) = self.picker {
                    let b = egui::Button::new(RichText::new(t("เลือก…")).size(14.0)).min_size(vec2(68.0, 36.0)).corner_radius(10);
                    if ui.add_enabled(self.picking.is_none(), b).clicked() {
                        let (tx, rx) = channel();
                        let start = PathBuf::from(self.dir.trim());
                        let ctx = ui.ctx().clone();
                        let _ = std::thread::Builder::new().name("ssnd-pick".into()).spawn(move || {
                            let _ = tx.send(inst::pick_folder(tool, &start));
                            ctx.request_repaint();
                        });
                        self.picking = Some(rx);
                    }
                }
            });
            ui.add_space(4.0);
            hint(ui, &format!("โปรแกรมจะเป็นไฟล์ชื่อ {} ในโฟลเดอร์นี้", inst::EXE_NAME));
        });
        if let Some(rx) = &self.picking {
            if let Ok(picked) = rx.try_recv() {
                if let Some(p) = picked {
                    self.dir = p.to_string_lossy().into_owned();
                }
                self.picking = None;
            }
        }
        ui.add_space(12.0);
        card(ui, |ui| {
            toggle_row(ui, "ทางลัดบนเดสก์ท็อป", None, &mut self.desktop_shortcut);
            ui.add_space(8.0);
            toggle_row(ui, "เพิ่มในเมนูแอป", Some("หา Stream Sound ได้จากเมนู และคลิกขวาที่ไอคอนเพื่อถอนการติดตั้ง"), &mut self.menu);
            ui.add_space(8.0);
            toggle_row(ui, "เปิด Stream Sound เมื่อติดตั้งเสร็จ", None, &mut self.launch);
        });
        ui.add_space(18.0);
        if big_button(ui, &self.g, if updating { "อัปเดต" } else { "ติดตั้ง" }, BigKind::Primary).clicked() {
            self.start(ui.ctx());
        }
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if link_button(ui, "ใช้เลยโดยไม่ติดตั้ง").clicked() {
                inst::remember_portable();
                if let Some(me) = inst::this_exe() {
                    let _ = inst::launch(&me, &[]);
                }
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn uninstall_page(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "ถอนการติดตั้ง Stream Sound", &Self::version());
        card(ui, |ui| {
            ui.label(RichText::new(t("จะลบ Stream Sound ออกจากเครื่องนี้ พร้อมไอคอน ทางลัด และการเปิดพร้อมเครื่อง")).size(14.5).color(TEXT));
            if let Some(e) = &self.existing {
                ui.add_space(4.0);
                hint(ui, &e.display().to_string());
            }
            ui.add_space(12.0);
            toggle_row(ui, "ลบการตั้งค่าด้วย", Some("ชื่อเครื่อง อุปกรณ์ที่เลือกไว้ และระดับเสียง"), &mut self.remove_settings);
        });
        ui.add_space(18.0);
        if big_button(ui, &self.g, "ถอนการติดตั้ง", BigKind::Danger).clicked() {
            self.start(ui.ctx());
        }
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if link_button(ui, "ยกเลิก").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn working_page(&mut self, ui: &mut egui::Ui) {
        let (title, sub) = match self.mode {
            Mode::Install => ("กำลังติดตั้ง Stream Sound", Self::version()),
            Mode::Update(_) => ("กำลังอัปเดต Stream Sound", format!("เป็น{}", Self::version())),
            Mode::Uninstall => ("กำลังถอนการติดตั้ง", String::new()),
        };
        self.header(ui, title, &sub);
        let (value, text) = {
            let p = self.progress.lock();
            (p.value, p.text.clone())
        };
        progress_bar(ui, &self.g, value);
        ui.add_space(10.0);
        ui.label(RichText::new(t(text)).size(13.5).color(TEXT2));
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }

    fn done_page(&mut self, ui: &mut egui::Ui) {
        let (title, note) = match self.mode {
            Mode::Install if self.launch => ("ติดตั้งเสร็จแล้ว", "กำลังเปิด Stream Sound"),
            Mode::Install if self.menu => ("ติดตั้งเสร็จแล้ว", "เปิด Stream Sound ได้จากเมนูแอป"),
            Mode::Install => ("ติดตั้งเสร็จแล้ว", "เปิด Stream Sound ได้จากทางลัดหรือโฟลเดอร์ที่ติดตั้ง"),
            Mode::Update(_) => ("อัปเดตเสร็จแล้ว", "กำลังเปิดเวอร์ชันใหม่"),
            Mode::Uninstall => ("ถอนการติดตั้งแล้ว", "ลบ Stream Sound ออกจากเครื่องนี้แล้ว"),
        };
        self.header(ui, title, note);
        progress_bar(ui, &self.g, 1.0);
        ui.add_space(18.0);
        if self.close_at.is_none() && big_button(ui, &self.g, "ปิด", BigKind::Primary).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn failed_page(&mut self, ui: &mut egui::Ui, error: &str) {
        let title = match self.mode {
            Mode::Install => "ติดตั้งไม่สำเร็จ",
            Mode::Update(_) => "อัปเดตไม่สำเร็จ",
            Mode::Uninstall => "ถอนการติดตั้งไม่สำเร็จ",
        };
        self.header(ui, title, "");
        egui::Frame::new()
            .fill(Color32::from_rgb(0x3A, 0x12, 0x18))
            .stroke(Stroke::new(1.0, ERROR))
            .corner_radius(12)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(t(error)).size(13.5).color(TEXT));
            });
        ui.add_space(18.0);
        if big_button(ui, &self.g, "ลองอีกครั้ง", BigKind::Primary).clicked() {
            match self.mode {
                Mode::Update(_) => self.start(ui.ctx()),
                _ => self.stage = Stage::Ask,
            }
        }
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if link_button(ui, "ปิด").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

impl eframe::App for Setup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Stage::Working = self.stage {
            if let Some(result) = self.progress.lock().result.take() {
                self.stage = match result {
                    Ok(()) => Stage::Done,
                    Err(e) => Stage::Failed(e),
                };
                // Nothing left to read when the app opens by itself.
                let auto = match self.mode {
                    Mode::Install => self.launch,
                    Mode::Update(_) => true,
                    Mode::Uninstall => false,
                };
                if matches!(self.stage, Stage::Done) && auto {
                    self.close_at = Some(Instant::now() + Duration::from_millis(900));
                }
            }
        }
        if let Some(at) = self.close_at {
            if Instant::now() >= at {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        let frame = egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(20));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| match &self.stage {
                    Stage::Ask => match self.mode {
                        Mode::Uninstall => self.uninstall_page(ui),
                        _ => self.install_page(ui),
                    },
                    Stage::Working => self.working_page(ui),
                    Stage::Done => self.done_page(ui),
                    Stage::Failed(e) => {
                        let e = e.clone();
                        self.failed_page(ui, &e)
                    }
                });
            });
        });
    }
}
