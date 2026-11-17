//! 執行層：把核心（判定邏輯）與平台層（藍牙、鎖定）組起來。
//!
//! 介面（Swift 原生視窗、egui 視窗……）只需要透過 [`Runtime`] 取得狀態、修改設定，
//! 不必知道藍牙或判定的細節。

pub mod calibrate;
pub mod config;
pub mod engine;
pub mod view;

use awaylock_core::ports::{Autostart, DeviceSource};
use awaylock_platform::{BleScanner, SystemAutostart, SystemLockProbe, SystemLocker};
use engine::Shared;
use std::sync::{Arc, Mutex};
pub struct Runtime {
    pub shared: Shared,
    pub source: Arc<dyn DeviceSource>,
    pub autostart: Arc<dyn Autostart>,
}

impl Runtime {
    /// 載入設定、開始藍牙掃描，並啟動背景檢查執行緒。
    pub fn start() -> Self {
        let shared = Arc::new(Mutex::new(engine::SharedState::new(
            config::load_settings(),
            config::load_allowlist(),
        )));
        let source: Arc<dyn DeviceSource> = Arc::new(BleScanner::start());
        engine::spawn(
            shared.clone(),
            source.clone(),
            Arc::new(SystemLocker),
            Arc::new(SystemLockProbe),
        );
        let autostart: Arc<dyn Autostart> = Arc::new(SystemAutostart::new("awaylock"));
        Self { shared, source, autostart }
    }

    /// 設定離開距離（公尺），自動限制範圍並對齊 0.5 公尺，然後存檔。
    pub fn set_distance(&self, meters: f64) {
        let mut st = self.shared.lock().unwrap();
        st.settings.set_distance(meters);
        config::save_settings(&st.settings);
    }

    /// 「附近的裝置」是否只顯示手機與手錶。
    ///
    /// 只有開發版能關閉過濾（看全部裝置）；發布版一律只顯示手機與手錶，這個函式不做任何事。
    pub fn set_only_phones_watches(&self, on: bool) {
        #[cfg(debug_assertions)]
        {
            let mut st = self.shared.lock().unwrap();
            st.settings.only_phones_watches = on;
            config::save_settings(&st.settings);
        }
        #[cfg(not(debug_assertions))]
        let _ = on;
    }

    pub fn add_device(&self, id: &str, name: &str) {
        let mut st = self.shared.lock().unwrap();
        if st.allowlist.add(id, name) {
            config::save_allowlist(&st.allowlist);
        }
    }

    /// 開始「在 1 公尺校正」：在背景收集數秒的讀數，進度與結果見狀態的 `calibration_message`。
    pub fn calibrate_device(&self, id: &str) -> calibrate::StartResult {
        calibrate::start(&self.shared, &self.source, id)
    }

    /// 清除校正，改回依裝置種類的預設值。
    pub fn reset_calibration(&self, id: &str) {
        calibrate::reset(&self.shared, id);
    }

    pub fn remove_device(&self, id: &str) {
        let mut st = self.shared.lock().unwrap();
        st.calibrations.remove(id);
        if st.allowlist.remove(id) {
            config::save_allowlist(&st.allowlist);
        }
    }
}
