//! 狀態列圖示（程式繪製的鎖頭，黑色樣板圖，系統會依選單列明暗自動變色）與選單。

use tray_icon::menu::{Menu, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const SIZE: u32 = 44;

/// 有號距離函式：圓角矩形。
fn rrect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hw + r;
    let qy = (py - cy).abs() - hh + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn lock_icon() -> Icon {
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let body = rrect(px, py, 22.0, 29.0, 13.0, 9.0, 3.0);
            let ring = ((px - 22.0).hypot(py - 19.0) - 7.75).abs() - 1.75;
            let shackle = ring.max(py - 22.0); // 只保留上半圈
            let union = body.min(shackle);
            let keyhole = ((px - 22.0).hypot(py - 27.0) - 2.5).min(rrect(px, py, 22.0, 31.0, 1.0, 3.5, 0.0));
            let d = union.max(-keyhole);
            let alpha = ((0.5 - d).clamp(0.0, 1.0) * 255.0) as u8;
            rgba.extend_from_slice(&[0, 0, 0, alpha]);
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("tray icon")
}

pub struct Tray {
    /// 持有狀態列圖示；被丟棄時圖示會消失，所以要一直保留。
    #[allow(dead_code)]
    pub icon: TrayIcon,
    pub settings_id: tray_icon::menu::MenuId,
    pub quit_id: tray_icon::menu::MenuId,
}

/// 建立狀態列圖示。選單只有 Settings 與 Quit。必須在事件迴圈啟動後呼叫。
pub fn build() -> Result<Tray, String> {
    let settings = MenuItem::new("Settings", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append(&settings).map_err(|e| e.to_string())?;
    menu.append(&quit).map_err(|e| e.to_string())?;

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(lock_icon())
        .with_icon_as_template(true)
        .with_tooltip("awaylock")
        .build()
        .map_err(|e| e.to_string())?;

    Ok(Tray { icon, settings_id: settings.id().clone(), quit_id: quit.id().clone() })
}
