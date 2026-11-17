//! 開機（登入）時自動啟動。

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use awaylock_core::ports::Autostart;
use std::path::PathBuf;

pub struct SystemAutostart {
    inner: Option<AutoLaunch>,
    /// 無法啟用時的原因（例如不是從 .app 啟動）。
    unavailable: Option<String>,
}

impl SystemAutostart {
    pub fn new(app_name: &str) -> Self {
        match app_path() {
            Ok(path) => {
                let built = AutoLaunchBuilder::new()
                    .set_app_name(app_name)
                    .set_app_path(&path.to_string_lossy())
                    .set_use_launch_agent(true)
                    .build();
                match built {
                    Ok(al) => Self { inner: Some(al), unavailable: None },
                    Err(e) => Self { inner: None, unavailable: Some(e.to_string()) },
                }
            }
            Err(e) => Self { inner: None, unavailable: Some(e) },
        }
    }
}

/// macOS 若在 .app 內執行，回傳 .app 路徑；否則回傳執行檔路徑。
fn app_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        // .../awaylock.app/Contents/MacOS/awaylock → .../awaylock.app
        if let Some(app) = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")) {
            return Ok(app.to_path_buf());
        }
    }
    Ok(exe)
}

impl Autostart for SystemAutostart {
    fn is_enabled(&self) -> bool {
        self.inner.as_ref().and_then(|a| a.is_enabled().ok()).unwrap_or(false)
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        let Some(al) = &self.inner else {
            return Err(self.unavailable.clone().unwrap_or_else(|| "無法設定開機啟動".into()));
        };
        let r = if enabled { al.enable() } else { al.disable() };
        r.map_err(|e| e.to_string())
    }
}
