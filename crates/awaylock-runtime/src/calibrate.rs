//! 「在 1 公尺校正」的執行流程：在背景收集數秒的讀數，結束後存進允許列表。
//! Swift 與 egui 介面共用這裡的邏輯與訊息文字。

use crate::config;
use crate::engine::Shared;
use awaylock_core::calibration::{CALIBRATION_DURATION, MAX_RSSI_AT_1M, MIN_RSSI_AT_1M};
use awaylock_core::ports::DeviceSource;
use awaylock_core::{Calibration, CalibrationError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 收集期間多久取一次裝置狀態。
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 某個裝置的校正進度。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalibrationStatus {
    Running,
    Done(Result<f64, CalibrationError>),
}

impl CalibrationStatus {
    /// 給使用者看的訊息。
    pub fn message(&self) -> String {
        match self {
            Self::Running => format!(
                "校正中：請讓裝置保持在離 Mac 約 1 公尺處，約 {} 秒…",
                CALIBRATION_DURATION.as_secs()
            ),
            Self::Done(Ok(_)) => "已校正：目前位置視為 1 公尺。".into(),
            Self::Done(Err(CalibrationError::NotEnoughReadings { .. })) => {
                "校正失敗：訊號斷斷續續，請確認藍牙已開啟、裝置靠近一點後再試。".into()
            }
            Self::Done(Err(CalibrationError::OutOfRange { rssi })) if *rssi < MIN_RSSI_AT_1M => {
                "校正失敗：訊號太弱，不像在 1 公尺處。請把裝置拿近到約 1 公尺後再試。".into()
            }
            Self::Done(Err(CalibrationError::OutOfRange { rssi })) if *rssi > MAX_RSSI_AT_1M => {
                "校正失敗：訊號太強，裝置可能太靠近 Mac。請拉開到約 1 公尺後再試。".into()
            }
            Self::Done(Err(CalibrationError::OutOfRange { .. })) => "校正失敗，請再試一次。".into(),
        }
    }
}

/// 開始校正的結果（數值即 C 介面的回傳碼）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum StartResult {
    Started = 0,
    /// 不在允許列表中。
    NotAllowed = 1,
    /// 最近沒收到這個裝置的廣播。
    NotSeen = 2,
    /// 這個裝置已經在校正中。
    AlreadyRunning = 3,
}

/// 開始校正：先檢查，再在背景執行緒收集 [`CALIBRATION_DURATION`] 的讀數。
pub fn start(shared: &Shared, source: &Arc<dyn DeviceSource>, id: &str) -> StartResult {
    let result = begin(shared, source.as_ref(), id);
    if result == StartResult::Started {
        let (shared, source, id) = (shared.clone(), source.clone(), id.to_string());
        std::thread::Builder::new()
            .name("awaylock-calibrate".into())
            .spawn(move || {
                let cal = collect(source.as_ref(), &id, CALIBRATION_DURATION, POLL_INTERVAL);
                finish(&shared, &id, &cal);
            })
            .expect("spawn calibration thread");
    }
    result
}

/// 檢查能否開始校正；可以就把狀態設為進行中。
pub fn begin(shared: &Shared, source: &dyn DeviceSource, id: &str) -> StartResult {
    let devices = source.snapshot();
    let mut st = shared.lock().unwrap();
    if !st.allowlist.contains(id) {
        return StartResult::NotAllowed;
    }
    if st.calibrations.get(id) == Some(&CalibrationStatus::Running) {
        return StartResult::AlreadyRunning;
    }
    let stale = st.monitor.config().stale_after;
    if devices.get(id).is_none_or(|d| Instant::now().saturating_duration_since(d.last_seen) >= stale) {
        // 記下結果，讓介面顯示「訊號斷斷續續」的提示
        let none = Err(CalibrationError::NotEnoughReadings { got: 0 });
        st.calibrations.insert(id.to_string(), CalibrationStatus::Done(none));
        return StartResult::NotSeen;
    }
    st.calibrations.insert(id.to_string(), CalibrationStatus::Running);
    StartResult::Started
}

/// 在 `duration` 內每 `interval` 取一次裝置狀態，收集新讀數。
pub fn collect(source: &dyn DeviceSource, id: &str, duration: Duration, interval: Duration) -> Calibration {
    let mut cal = Calibration::new();
    let start = Instant::now();
    loop {
        let now = Instant::now();
        if let Some(d) = source.snapshot().get(id) {
            cal.observe(d, now);
        }
        if now.saturating_duration_since(start) >= duration {
            return cal;
        }
        std::thread::sleep(interval);
    }
}

/// 結束校正：成功就存進允許列表。期間若已取消（重設或移除裝置）則不套用。
pub fn finish(shared: &Shared, id: &str, cal: &Calibration) {
    let mut st = shared.lock().unwrap();
    if st.calibrations.get(id) != Some(&CalibrationStatus::Running) {
        return;
    }
    let result = cal.finish();
    if let Ok(rssi) = result {
        if st.allowlist.set_calibration(id, Some(rssi)) {
            config::save_allowlist(&st.allowlist);
        }
    }
    st.calibrations.insert(id.to_string(), CalibrationStatus::Done(result));
}

/// 清除裝置的校正值與進度（也會讓進行中的校正不再套用）。
pub fn reset(shared: &Shared, id: &str) {
    let mut st = shared.lock().unwrap();
    st.calibrations.remove(id);
    if st.allowlist.get(id).is_some_and(|a| a.rssi_at_1m.is_some()) {
        st.allowlist.set_calibration(id, None);
        config::save_allowlist(&st.allowlist);
    }
}
