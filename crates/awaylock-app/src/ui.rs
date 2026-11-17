//! 設定視窗（egui）。分頁：General / Devices / Debug（僅開發版）/ About。

use awaylock_runtime::calibrate::{self, CalibrationStatus};
use awaylock_runtime::config;
use awaylock_runtime::engine::Shared;
use crate::tray::{self, Tray};
use awaylock_core::distance::format_meters;
use awaylock_core::ports::{Autostart, DeviceSource};
use awaylock_core::NearbyDevice;
#[cfg(debug_assertions)]
use awaylock_core::Phase;
use eframe::egui::{self, Align, Color32, Layout, RichText, Vec2, ViewportCommand};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

pub const WINDOW_TITLE: &str = "awaylock – Settings";
const STALE: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    General,
    Devices,
    #[cfg(debug_assertions)]
    Debug,
    About,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::Devices => "Devices",
            #[cfg(debug_assertions)]
            Tab::Debug => "Debug",
            Tab::About => "About",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Tab::General => "⚙",
            Tab::Devices => "📶",
            #[cfg(debug_assertions)]
            Tab::Debug => "🐞",
            Tab::About => "ℹ",
        }
    }

    /// 視窗大小（含分頁列）。
    pub fn size(self) -> Vec2 {
        match self {
            Tab::General => Vec2::new(560.0, 430.0),
            Tab::Devices => Vec2::new(560.0, 520.0),
            #[cfg(debug_assertions)]
            Tab::Debug => Vec2::new(560.0, 600.0),
            Tab::About => Vec2::new(560.0, 330.0),
        }
    }
}

pub fn initial_size() -> Vec2 {
    Tab::General.size()
}

fn visible_tabs(_debug_on: bool) -> Vec<Tab> {
    let mut tabs = vec![Tab::General, Tab::Devices];
    #[cfg(debug_assertions)]
    if _debug_on {
        tabs.push(Tab::Debug);
    }
    tabs.push(Tab::About);
    tabs
}

enum Cmd {
    ShowSettings,
}

pub struct App {
    shared: Shared,
    source: Arc<dyn DeviceSource>,
    autostart: Arc<dyn Autostart>,
    tab: Tab,
    cmd_rx: mpsc::Receiver<Cmd>,
    _tray: Tray,
    launch_at_login: bool,
    autostart_error: Option<String>,
}

impl App {
    pub fn new(
        ctx: egui::Context,
        shared: Shared,
        source: Arc<dyn DeviceSource>,
        autostart: Arc<dyn Autostart>,
    ) -> Result<Self, String> {
        setup_fonts(&ctx);

        let tray = tray::build()?;
        let (tx, rx) = mpsc::channel();
        let settings_id = tray.settings_id.clone();
        let quit_id = tray.quit_id.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |ev: tray_icon::menu::MenuEvent| {
            if ev.id == quit_id {
                std::process::exit(0);
            }
            if ev.id == settings_id {
                let _ = tx.send(Cmd::ShowSettings);
                ctx.request_repaint(); // 視窗隱藏時也要喚醒 update
            }
        }));

        let launch_at_login = autostart.is_enabled();
        Ok(Self {
            shared,
            source,
            autostart,
            tab: Tab::General,
            cmd_rx: rx,
            _tray: tray,
            launch_at_login,
            autostart_error: None,
        })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                Cmd::ShowSettings => {
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                }
            }
        }
        // 關閉視窗只是隱藏，App 繼續留在狀態列
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        let settings_before = self.shared.lock().unwrap().settings.clone();

        #[cfg(debug_assertions)]
        let debug_on = settings_before.debug_mode;
        #[cfg(not(debug_assertions))]
        let debug_on = false;

        let tabs = visible_tabs(debug_on);
        if !tabs.contains(&self.tab) {
            self.tab = Tab::General;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(Tab::General.size()));
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let total = tabs.len() as f32 * 90.0;
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                for t in &tabs {
                    let text = RichText::new(format!("{}\n{}", t.icon(), t.label()));
                    let resp = ui.add_sized([84.0, 52.0], egui::Button::new(text).selected(self.tab == *t));
                    if resp.clicked() && self.tab != *t {
                        self.tab = *t;
                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(t.size()));
                    }
                }
            });
            ui.add_space(6.0);
        });

        if self.tab == Tab::General {
            egui::TopBottomPanel::bottom("quit_bar").show(ctx, |ui| {
                ui.add_space(10.0);
                // 用 horizontal 包起來限制高度，否則 right_to_left 會佔滿所有剩餘高度
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(24.0);
                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });
                });
                ui.add_space(10.0);
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::General => self.general_tab(ui),
            Tab::Devices => self.devices_tab(ui),
            #[cfg(debug_assertions)]
            Tab::Debug => self.debug_tab(ui),
            Tab::About => about_tab(ui),
        });

        let settings_after = self.shared.lock().unwrap().settings.clone();
        if settings_after != settings_before {
            config::save_settings(&settings_after);
        }

        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(ui.visuals().weak_text_color()));
}

impl App {
    fn general_tab(&mut self, ui: &mut egui::Ui) {
        let shared = self.shared.clone();
        ui.add_space(16.0);
        ui.horizontal_top(|ui| {
            ui.add_space(70.0);
            ui.vertical(|ui| {
                ui.set_width(420.0);

                // 開機時啟動
                let mut on = self.launch_at_login;
                if ui.checkbox(&mut on, "開機時啟動").changed() {
                    match self.autostart.set_enabled(on) {
                        Ok(()) => {
                            self.launch_at_login = on;
                            self.autostart_error = None;
                        }
                        Err(e) => self.autostart_error = Some(format!("無法變更：{e}")),
                    }
                }
                ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    caption(ui, "登入 Mac 時自動開啟 awaylock。");
                });
                if let Some(e) = &self.autostart_error {
                    ui.colored_label(Color32::LIGHT_RED, e);
                }

                ui.add_space(18.0);

                // 離開距離
                let mut st = shared.lock().unwrap();
                ui.label(format!("離開距離：約 {:.1} 公尺", st.settings.distance_m));
                ui.horizontal(|ui| {
                    ui.label("近");
                    let mut d = st.settings.distance_m;
                    ui.spacing_mut().slider_width = 330.0;
                    let slider = egui::Slider::new(&mut d, 1.0..=5.0).step_by(0.5).show_value(false);
                    if ui.add(slider).changed() {
                        st.settings.set_distance(d);
                    }
                    ui.label("遠");
                });
                caption(
                    ui,
                    "電腦解鎖期間掃描到過的允許裝置，只要有任何一個超過這個距離或突然掃描不到，就會自動鎖定螢幕。\
                     距離是由藍牙訊號粗估，會受牆壁和遮擋影響，僅供參考。",
                );

                #[cfg(debug_assertions)]
                {
                    ui.add_space(18.0);
                    let mut dbg = st.settings.debug_mode;
                    if ui.checkbox(&mut dbg, "Debug 模式").changed() {
                        st.settings.debug_mode = dbg;
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        caption(ui, "僅開發版：顯示 Debug 分頁，列出掃描與判定細節。");
                    });
                }
            });
        });
    }

    fn devices_tab(&mut self, ui: &mut egui::Ui) {
        let devices = self.source.snapshot();
        let status = self.source.status_text();
        let now = Instant::now();
        let mut nearby: Vec<&NearbyDevice> = devices.values().collect();
        nearby.sort_by(|a, b| b.rssi.cmp(&a.rssi));

        let mut to_add: Vec<(String, String)> = vec![];
        let mut to_remove: Vec<String> = vec![];
        let mut to_calibrate: Option<String> = None;
        let mut to_reset: Option<String> = None;

        {
            let st = self.shared.lock().unwrap();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("允許的裝置");
                if st.allowlist.is_empty() {
                    caption(ui, "尚未加入任何裝置。加入後，只有這些裝置離開時才會鎖定。");
                }
                for a in st.allowlist.devices() {
                    let seen_dev =
                        devices.get(&a.id).filter(|d| now.saturating_duration_since(d.last_seen) < STALE);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(&a.name);
                            let mut text = match seen_dev {
                                Some(d) => format!("在附近 · 約 {}", format_meters(d.meters_calibrated(a.rssi_at_1m))),
                                None => "未偵測到".to_string(),
                            };
                            if a.rssi_at_1m.is_some() {
                                text.push_str(" · 已校正");
                            }
                            caption(ui, &text);
                            if let Some(status) = st.calibrations.get(&a.id) {
                                caption(ui, &status.message());
                            }
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("移除").clicked() {
                                to_remove.push(a.id.clone());
                            }
                            let calibrating = st.calibrations.get(&a.id).is_some_and(CalibrationStatus::is_running);
                            let label = if calibrating { "校正中…" } else { "在 1 公尺校正" };
                            if ui
                                .add_enabled(seen_dev.is_some() && !calibrating, egui::Button::new(label))
                                .on_hover_text("把裝置放在離 Mac 約 1 公尺處，按下後保持不動幾秒")
                                .clicked()
                            {
                                to_calibrate = Some(a.id.clone());
                            }
                            if a.rssi_at_1m.is_some()
                                && ui.add_enabled(!calibrating, egui::Button::new("重設校正")).clicked()
                            {
                                to_reset = Some(a.id.clone());
                            }
                        });
                    });
                    ui.separator();
                }
                if !st.allowlist.is_empty() {
                    caption(ui, "距離不準時：把裝置放在離 Mac 約 1 公尺處，按「在 1 公尺校正」後保持不動；iPhone 請先解鎖並保持螢幕亮著。");
                }

                ui.add_space(12.0);
                ui.heading(format!("附近的裝置（{status}）"));
                for d in nearby.iter().filter(|d| !st.allowlist.contains(&d.id)) {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(d.display_name());
                            let apple = if d.is_apple { " · Apple" } else { "" };
                            caption(ui, &format!("約 {}{apple}", format_meters(d.meters())));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("加入").clicked() {
                                to_add.push((d.id.clone(), d.display_name()));
                            }
                        });
                    });
                    ui.separator();
                }
                caption(
                    ui,
                    "提示：把手機或手錶靠近 Mac，距離最近的通常就是它。\
                     Apple 裝置的識別碼會定期更換，更換後需要重新加入。",
                );
            });
        }

        if let Some(id) = to_calibrate {
            calibrate::start(&self.shared, &self.source, &id);
        }
        if let Some(id) = to_reset {
            calibrate::reset(&self.shared, &id);
        }

        if !to_add.is_empty() || !to_remove.is_empty() {
            let mut st = self.shared.lock().unwrap();
            for (id, name) in to_add {
                st.allowlist.add(id, name);
            }
            for id in to_remove {
                st.calibrations.remove(&id);
                st.allowlist.remove(&id);
            }
            config::save_allowlist(&st.allowlist);
        }
    }

    #[cfg(debug_assertions)]
    fn debug_tab(&mut self, ui: &mut egui::Ui) {
        let devices = self.source.snapshot();
        let status = self.source.status_text();
        let now = Instant::now();
        let st = self.shared.lock().unwrap();
        let m = &st.monitor;
        let th = st.settings.threshold();

        let detail = |d: Option<&NearbyDevice>| -> String {
            let cal = d.and_then(|d| st.allowlist.get(&d.id)).and_then(|a| a.rssi_at_1m);
            match d {
                None => "無資料".to_string(),
                Some(d) => format!(
                    "{} dBm · 約 {} · {} 秒前",
                    d.rssi,
                    format_meters(d.meters_calibrated(cal)),
                    now.saturating_duration_since(d.last_seen).as_secs()
                ),
            }
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("離開判定");
            egui::Grid::new("dbg_grid").num_columns(2).striped(true).show(ui, |ui| {
                let mut row = |k: &str, v: String| {
                    ui.label(k);
                    ui.label(RichText::new(v).weak());
                    ui.end_row();
                };
                row("離開距離", format!("{:.1} 公尺 ≈ {} dBm", st.settings.distance_m, th));
                row("Mac 藍牙", status.clone());
                row(
                    "電腦狀態",
                    match m.phase() {
                        Phase::Unlocked => "已解鎖",
                        Phase::AwaitingLock { .. } => "等待鎖定生效",
                        Phase::Locked => "已鎖定",
                    }
                    .into(),
                );
                row("本次解鎖期間見過的允許裝置", format!("{} / {}", m.armed().len(), st.allowlist.devices().len()));
                row(
                    "連續離開次數（監控中裝置）",
                    format!("{} / {}", m.max_misses(), m.config().required_misses),
                );
                row(
                    "最近檢查",
                    st.last_check
                        .map(|t| format!("{} 秒前", now.saturating_duration_since(t).as_secs()))
                        .unwrap_or_else(|| "—".into()),
                );
                row("判定", m.decision().to_string());
                row(
                    "檢查間隔 / 失聯判定",
                    format!("{} 秒 / {} 秒", awaylock_runtime::engine::TICK_INTERVAL.as_secs(), m.config().stale_after.as_secs()),
                );
            });

            ui.add_space(10.0);
            ui.heading("允許的裝置（即時）");
            if st.allowlist.is_empty() {
                caption(ui, "無");
            }
            for a in st.allowlist.devices() {
                let p = m.evaluate(a, &devices, th, now);
                ui.horizontal(|ui| {
                    ui.label(&a.name);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (t, c) = if p.present { ("在範圍內", Color32::LIGHT_GREEN) } else { ("離開", Color32::LIGHT_RED) };
                        ui.colored_label(c, t);
                    });
                });
                let armed = if m.is_armed(&a.id) { "監控中" } else { "尚未見過" };
                caption(ui, &format!("{} · {} · {}", detail(devices.get(&a.id)), p.reason, armed));
                caption(ui, &a.id);
                ui.separator();
            }

            ui.add_space(10.0);
            let mut all: Vec<&NearbyDevice> = devices.values().collect();
            all.sort_by(|a, b| b.rssi.cmp(&a.rssi));
            ui.heading(format!("掃描到的所有裝置（{}）", all.len()));
            for d in all {
                ui.horizontal(|ui| {
                    ui.label(d.display_name());
                    if st.allowlist.contains(&d.id) {
                        ui.colored_label(Color32::LIGHT_GREEN, "✔");
                    }
                    if d.is_apple {
                        ui.label(RichText::new("Apple").weak());
                    }
                });
                caption(ui, &detail(Some(d)));
                caption(ui, &d.id);
                ui.separator();
            }
        });
    }
}

fn about_tab(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        ui.label(RichText::new("🔒").size(40.0));
        ui.label(RichText::new("awaylock").size(22.0).strong());
        ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak());
        ui.add_space(8.0);
        ui.label("iPhone / Apple Watch 離開藍牙範圍時，自動鎖定 Mac。");
    });
}

/// egui 預設字型沒有中日韓字，從系統載入一個備援字型。
fn setup_fonts(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        // macOS
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/Songti.ttc",
        // Linux
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("cjk".into(), egui::FontData::from_owned(bytes));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("cjk".into());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}
