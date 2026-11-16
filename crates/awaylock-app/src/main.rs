//! AwayLock 跨平台備用介面（egui）：狀態列圖示與設定視窗。
//!
//! macOS 的正式介面是 `apps/macos` 的 Swift 原生視窗；這個程式預留給 Linux 等沒有原生介面的平台，
//! 在 macOS 上也能執行（外觀非原生）。

mod tray;
mod ui;

use awaylock_runtime::Runtime;

fn main() -> eframe::Result<()> {
    let rt = Runtime::start();
    let (shared, source, autostart) = (rt.shared.clone(), rt.source.clone(), rt.autostart.clone());

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(ui::WINDOW_TITLE)
            .with_inner_size(ui::initial_size())
            .with_resizable(false)
            .with_maximize_button(false)
            .with_minimize_button(false),
        ..Default::default()
    };

    eframe::run_native(
        "AwayLock",
        options,
        Box::new(move |cc| {
            let app = ui::App::new(cc.egui_ctx.clone(), shared, source, autostart)?;
            Ok(Box::new(app))
        }),
    )
}
