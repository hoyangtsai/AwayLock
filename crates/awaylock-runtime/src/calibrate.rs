//! 「在 1 公尺校正」的執行流程：在背景收集裝置的廣播，結束後存進允許列表。
//! Swift 與 egui 介面共用這裡的邏輯與訊息文字。

use crate::config;
use crate::engine::Shared;
use awaylock_core::calibration::{
    MAX_CALIBRATION_TIME, MAX_RSSI_AT_1M, MIN_CALIBRATION_TIME, MIN_READINGS, MIN_RSSI_AT_1M,
};
use awaylock_core::ports::DeviceSource;
use awaylock_core::{Calibration, CalibrationError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 收集期間多久取一次裝置狀態。
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 某個裝置的校正進度。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalibrationStatus {
    /// 收集中，`got` 是已收到的廣播次數。
    Running { got: usize },
    Done(Result<f64, CalibrationError>),
}

impl CalibrationStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// 給使用者看的訊息。
    pub fn message(&self) -> String {
        match self {
            Self::Running { got } => format!(
                "校正中：已收到 {got}/{MIN_READINGS} 次訊號，請讓裝置保持在離 Mac 約 1 公尺處（最多 {} 秒）…",
                MAX_CALIBRATION_TIME.as_secs()
            ),
            Self::Done(Ok(_)) => "已校正：目前位置視為 1 公尺。".into(),
            Self::Done(Err(CalibrationError::NotEnoughReadings { got: 0 })) => {
                "校正失敗：沒有收到這個裝置的訊號，請確認藍牙已開啟、裝置在附近後再試。".into()
            }
            Self::Done(Err(CalibrationError::NotEnoughReadings { got })) => format!(
                "校正失敗：{} 秒內只收到 {got} 次訊號（需要 {MIN_READINGS} 次）。\
                 iPhone 鎖定時很少發送藍牙訊號，請解鎖並保持螢幕亮著再試。",
                MAX_CALIBRATION_TIME.as_secs()
            ),
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

/// 開始校正：先檢查，再在背景執行緒收集廣播（見 [`collect`]）。
pub fn start(shared: &Shared, source: &Arc<dyn DeviceSource>, id: &str) -> StartResult {
    let result = begin(shared, source.as_ref(), id);
    if result == StartResult::Started {
        let (shared, source, id) = (shared.clone(), source.clone(), id.to_string());
        std::thread::Builder::new()
            .name("awaylock-calibrate".into())
            .spawn(move || {
                let cal = collect(
                    &shared,
                    source.as_ref(),
                    &id,
                    MIN_CALIBRATION_TIME,
                    MAX_CALIBRATION_TIME,
                    POLL_INTERVAL,
                );
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
    if st.calibrations.get(id).is_some_and(CalibrationStatus::is_running) {
        return StartResult::AlreadyRunning;
    }
    let stale = st.monitor.config().stale_after;
    if devices.get(id).is_none_or(|d| Instant::now().saturating_duration_since(d.last_seen) >= stale) {
        // 記下結果，讓介面顯示「訊號斷斷續續」的提示
        let none = Err(CalibrationError::NotEnoughReadings { got: 0 });
        st.calibrations.insert(id.to_string(), CalibrationStatus::Done(none));
        return StartResult::NotSeen;
    }
    st.calibrations.insert(id.to_string(), CalibrationStatus::Running { got: 0 });
    StartResult::Started
}

/// 每 `interval` 取一次裝置狀態，記錄按下之後的每一次廣播，並更新進度。
/// 收滿 `min_time` 且廣播次數足夠，或到了 `max_time` 就停止；期間被取消（重設、移除）也會提早停止。
pub fn collect(
    shared: &Shared,
    source: &dyn DeviceSource,
    id: &str,
    min_time: Duration,
    max_time: Duration,
    interval: Duration,
) -> Calibration {
    let mut cal = Calibration::new(Instant::now());
    loop {
        let now = Instant::now();
        if let Some(d) = source.snapshot().get(id) {
            cal.observe(d);
        }
        {
            let mut st = shared.lock().unwrap();
            match st.calibrations.get_mut(id) {
                Some(CalibrationStatus::Running { got }) => *got = cal.readings(),
                _ => return cal, // 已取消
            }
        }
        if cal.is_complete(now, min_time, max_time) {
            return cal;
        }
        std::thread::sleep(interval);
    }
}

/// 結束校正：成功就存進允許列表。期間若已取消（重設或移除裝置）則不套用。
pub fn finish(shared: &Shared, id: &str, cal: &Calibration) {
    let mut st = shared.lock().unwrap();
    if !st.calibrations.get(id).is_some_and(CalibrationStatus::is_running) {
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
