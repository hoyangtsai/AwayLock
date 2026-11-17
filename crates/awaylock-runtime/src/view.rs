//! 給介面使用的狀態檢視（可序列化成 JSON）。
//!
//! 一般狀態（[`StateView`]）只含公尺距離，不含 dBm。
//! 除錯狀態（[`DebugView`]）只存在 debug 編譯；發布版沒有這些程式碼。

use crate::calibrate::CalibrationStatus;
use crate::Runtime;
use awaylock_core::{DeviceKind, NearbyDevice};
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct AllowedView {
    pub id: String,
    pub name: String,
    /// 最近 15 秒內有收到廣播。
    pub seen: bool,
    pub distance_m: Option<f64>,
    /// 使用者已在 1 公尺處校正過。
    pub calibrated: bool,
    /// 正在收集校正讀數。
    pub calibrating: bool,
    /// 最近一次校正的進度或結果訊息。
    pub calibration_message: Option<String>,
}

#[derive(Serialize)]
pub struct NearbyView {
    pub id: String,
    pub name: String,
    pub is_apple: bool,
    pub distance_m: f64,
}

#[derive(Serialize)]
pub struct StateView {
    pub distance_m: f64,
    pub bluetooth_status: String,
    pub bluetooth_ready: bool,
    /// 「附近的裝置」是否只顯示手機與手錶。
    pub only_phones_watches: bool,
    pub allowed: Vec<AllowedView>,
    /// 附近裝置（已在允許列表的不含；開啟過濾時只含手機與手錶），依名稱排序。
    /// 排序與訊號、距離無關，所以人走動時列表的順序不會跳動。
    pub nearby: Vec<NearbyView>,
    /// 因過濾而隱藏的裝置數。
    pub hidden_count: usize,
}

/// 依顯示名稱排序（不分大小寫），名稱相同時依識別碼，讓順序固定、不受訊號變動影響。
fn sort_by_name(list: &mut [&NearbyDevice]) {
    list.sort_by_cached_key(|d| (d.display_name().to_lowercase(), d.id.clone()));
}

impl Runtime {
    pub fn state(&self) -> StateView {
        let devices = self.source.snapshot();
        let now = Instant::now();
        let st = self.shared.lock().unwrap();
        let stale = st.monitor.config().stale_after;

        let allowed = st
            .allowlist
            .devices()
            .iter()
            .map(|a| {
                let seen_dev = devices
                    .get(&a.id)
                    .filter(|d| now.saturating_duration_since(d.last_seen) < stale);
                AllowedView {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    seen: seen_dev.is_some(),
                    distance_m: seen_dev.map(|d| d.meters_calibrated(a.rssi_at_1m)),
                    calibrated: a.rssi_at_1m.is_some(),
                    calibrating: st.calibrations.get(&a.id).is_some_and(CalibrationStatus::is_running),
                    calibration_message: st.calibrations.get(&a.id).map(CalibrationStatus::message),
                }
            })
            .collect();

        // 發布版固定只顯示手機與手錶；只有開發版可以選擇看全部
        let only_phones = if cfg!(debug_assertions) { st.settings.only_phones_watches } else { true };
        let mut others: Vec<&NearbyDevice> =
            devices.values().filter(|d| !st.allowlist.contains(&d.id)).collect();
        sort_by_name(&mut others);
        let total = others.len();
        let nearby: Vec<NearbyView> = others
            .into_iter()
            .filter(|d| !only_phones || d.kind() == DeviceKind::PhoneOrWatch)
            .map(|d| NearbyView {
                id: d.id.clone(),
                name: d.display_name(),
                is_apple: d.is_apple,
                distance_m: d.meters(),
            })
            .collect();
        let hidden_count = total - nearby.len();

        StateView {
            distance_m: st.settings.distance_m,
            bluetooth_status: self.source.status_text(),
            bluetooth_ready: self.source.bluetooth_ready(),
            only_phones_watches: only_phones,
            allowed,
            nearby,
            hidden_count,
        }
    }

    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.state()).unwrap_or_else(|_| "{}".into())
    }

    /// 除錯狀態 JSON。發布版固定回傳 `{}`。
    pub fn debug_json(&self) -> String {
        #[cfg(debug_assertions)]
        {
            serde_json::to_string(&debug::build(self)).unwrap_or_else(|_| "{}".into())
        }
        #[cfg(not(debug_assertions))]
        {
            "{}".into()
        }
    }
}

#[cfg(debug_assertions)]
mod debug {
    use super::*;
    use awaylock_core::Phase;

    #[derive(Serialize)]
    pub struct DebugAllowed {
        pub id: String,
        pub name: String,
        pub present: bool,
        pub reason: String,
        pub armed: bool,
        pub rssi: Option<i32>,
        pub distance_m: Option<f64>,
        pub age_secs: Option<u64>,
    }

    #[derive(Serialize)]
    pub struct DebugDevice {
        pub id: String,
        pub name: String,
        pub is_apple: bool,
        /// 推測的裝置種類（手機/手錶、其他、未知）。
        pub kind: String,
        /// Apple 廣播訊息類型（十六進位），例如「0C 10」。
        pub apple_types: String,
        pub allowed: bool,
        pub rssi: i32,
        pub distance_m: f64,
        pub age_secs: u64,
    }

    #[derive(Serialize)]
    pub struct DebugView {
        pub threshold_dbm: i32,
        pub distance_m: f64,
        pub bluetooth_status: String,
        pub phase: String,
        pub armed_count: usize,
        pub allowed_count: usize,
        pub max_misses: u32,
        pub required_misses: u32,
        pub last_check_secs: Option<u64>,
        pub decision: String,
        pub tick_secs: u64,
        pub stale_secs: u64,
        pub allowed: Vec<DebugAllowed>,
        pub all: Vec<DebugDevice>,
    }

    pub fn build(rt: &Runtime) -> DebugView {
        let devices = rt.source.snapshot();
        let now = Instant::now();
        let st = rt.shared.lock().unwrap();
        let m = &st.monitor;
        let th = st.settings.threshold();

        let allowed = st
            .allowlist
            .devices()
            .iter()
            .map(|a| {
                let p = m.evaluate(a, &devices, th, now);
                let d = devices.get(&a.id);
                DebugAllowed {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    present: p.present,
                    reason: p.reason,
                    armed: m.is_armed(&a.id),
                    rssi: d.map(|d| d.rssi),
                    distance_m: d.map(|d| d.meters_calibrated(a.rssi_at_1m)),
                    age_secs: d.map(|d| now.saturating_duration_since(d.last_seen).as_secs()),
                }
            })
            .collect();

        let mut all: Vec<&NearbyDevice> = devices.values().collect();
        super::sort_by_name(&mut all);
        let all = all
            .into_iter()
            .map(|d| DebugDevice {
                id: d.id.clone(),
                name: d.display_name(),
                is_apple: d.is_apple,
                kind: d.kind().label().to_string(),
                apple_types: d.apple_types.iter().map(|t| format!("{t:02X}")).collect::<Vec<_>>().join(" "),
                allowed: st.allowlist.contains(&d.id),
                rssi: d.rssi,
                distance_m: d.meters_calibrated(st.allowlist.get(&d.id).and_then(|a| a.rssi_at_1m)),
                age_secs: now.saturating_duration_since(d.last_seen).as_secs(),
            })
            .collect();

        DebugView {
            threshold_dbm: th,
            distance_m: st.settings.distance_m,
            bluetooth_status: rt.source.status_text(),
            phase: match m.phase() {
                Phase::Unlocked => "已解鎖",
                Phase::AwaitingLock { .. } => "等待鎖定生效",
                Phase::Locked => "已鎖定",
            }
            .into(),
            armed_count: m.armed().len(),
            allowed_count: st.allowlist.devices().len(),
            max_misses: m.max_misses(),
            required_misses: m.config().required_misses,
            last_check_secs: st.last_check.map(|t| now.saturating_duration_since(t).as_secs()),
            decision: m.decision().to_string(),
            tick_secs: crate::engine::TICK_INTERVAL.as_secs(),
            stale_secs: m.config().stale_after.as_secs(),
            allowed,
            all,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::SharedState;
    use awaylock_core::ports::{Autostart, DeviceSource};
    use awaylock_core::{AllowList, Settings};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct FakeSource(HashMap<String, NearbyDevice>);
    impl DeviceSource for FakeSource {
        fn bluetooth_ready(&self) -> bool {
            true
        }
        fn status_text(&self) -> String {
            "掃描中".into()
        }
        fn snapshot(&self) -> HashMap<String, NearbyDevice> {
            self.0.clone()
        }
    }

    struct NoAutostart;
    impl Autostart for NoAutostart {
        fn is_enabled(&self) -> bool {
            false
        }
        fn set_enabled(&self, _: bool) -> Result<(), String> {
            Ok(())
        }
    }

    fn runtime() -> Runtime {
        runtime_with(-70, -65)
    }

    /// `other_rssi`、`buds_rssi` 讓測試能改變訊號強弱，驗證排序不受影響。
    fn runtime_with(other_rssi: i32, buds_rssi: i32) -> Runtime {
        // 所有測試都用暫存資料夾，絕不寫入使用者的真實設定
        std::env::set_var("AWAYLOCK_CONFIG_DIR", std::env::temp_dir().join("awaylock-test-config"));
        let now = Instant::now();
        let mut devs = HashMap::new();
        devs.insert("mine".into(), NearbyDevice::new("mine".into(), Some("My Watch".into()), -59, true, now));
        devs.insert("other".into(), NearbyDevice::new("other".into(), Some("Pixel Phone".into()), other_rssi, false, now));
        // 耳機：不是手機或手錶，預設會被過濾掉
        devs.insert("buds".into(), NearbyDevice::new("buds".into(), Some("AirPods Pro".into()), buds_rssi, true, now));
        // 沒有任何線索的裝置
        devs.insert("mystery".into(), NearbyDevice::new("mystery".into(), None, -80, false, now));
        let mut allow = AllowList::default();
        allow.add("mine", "My Watch");
        Runtime {
            shared: Arc::new(Mutex::new(SharedState::new(Settings::default(), allow))),
            source: Arc::new(FakeSource(devs)),
            autostart: Arc::new(NoAutostart),
        }
    }

    #[test]
    fn state_has_distance_but_never_dbm() {
        let rt = runtime();
        let json = rt.state_json();
        assert!(json.contains("\"distance_m\""));
        assert!(!json.contains("rssi"), "{json}");
        assert!(!json.to_lowercase().contains("dbm"), "{json}");

        let s = rt.state();
        assert_eq!(s.allowed.len(), 1);
        assert!(s.allowed[0].seen);
        // -59 dBm ≈ 1 公尺
        assert!((s.allowed[0].distance_m.unwrap() - 1.0).abs() < 0.01);
        // 已允許的不出現在「附近」；預設只顯示手機與手錶，耳機和未知裝置被隱藏
        assert!(s.only_phones_watches);
        assert_eq!(s.nearby.len(), 1);
        assert_eq!(s.nearby[0].id, "other");
        assert_eq!(s.hidden_count, 2);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn filter_can_be_turned_off_in_dev_builds() {
        let rt = runtime();
        rt.set_only_phones_watches(false);
        let s = rt.state();
        assert!(!s.only_phones_watches);
        assert_eq!(s.nearby.len(), 3);
        assert_eq!(s.hidden_count, 0);
        // 依名稱排序：AirPods Pro、Pixel Phone、未命名裝置 mystery
        let ids: Vec<&str> = s.nearby.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["buds", "other", "mystery"]);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn order_does_not_change_with_signal_strength() {
        let order = |other: i32, buds: i32| -> Vec<String> {
            let rt = runtime_with(other, buds);
            rt.set_only_phones_watches(false);
            rt.state().nearby.into_iter().map(|d| d.id).collect()
        };
        // 誰比較近都不影響順序
        assert_eq!(order(-40, -90), order(-90, -40));
        assert_eq!(order(-40, -90), vec!["buds", "other", "mystery"]);
    }

    #[test]
    fn mutations_update_state() {
        let rt = runtime();
        rt.set_distance(2.3);
        assert_eq!(rt.state().distance_m, 2.5);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn filter_cannot_be_turned_off_in_release() {
        let rt = runtime();
        rt.set_only_phones_watches(false);
        let s = rt.state();
        assert!(s.only_phones_watches);
        assert_eq!(s.nearby.len(), 1);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_json_has_details_in_dev_builds() {
        let rt = runtime();
        let json = rt.debug_json();
        assert!(json.contains("threshold_dbm"));
        assert!(json.contains("\"armed\""));
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn debug_json_is_empty_in_release() {
        assert_eq!(runtime().debug_json(), "{}");
    }

    /// 每次取快照都前進 300 ms、收到一筆新廣播的假來源，用來模擬校正期間的讀數。
    struct LiveSource {
        dev: Mutex<NearbyDevice>,
        rssi: i32,
    }
    impl DeviceSource for LiveSource {
        fn bluetooth_ready(&self) -> bool {
            true
        }
        fn status_text(&self) -> String {
            "掃描中".into()
        }
        fn snapshot(&self) -> HashMap<String, NearbyDevice> {
            let mut d = self.dev.lock().unwrap();
            d.update(self.rssi, None, true, Instant::now());
            HashMap::from([(d.id.clone(), d.clone())])
        }
    }

    fn live_runtime(rssi: i32) -> Runtime {
        let rt = runtime();
        let dev = NearbyDevice::new("mine".into(), Some("My Watch".into()), rssi, true, Instant::now());
        Runtime { source: Arc::new(LiveSource { dev: Mutex::new(dev), rssi }), ..rt }
    }

    /// 同步跑完整個校正流程（不開執行緒、間隔 60 ms 讓每次快照都是新樣本）。
    fn run_calibration(rt: &Runtime, id: &str) -> crate::calibrate::StartResult {
        use crate::calibrate::{begin, collect, finish, StartResult};
        let r = begin(&rt.shared, rt.source.as_ref(), id);
        if r == StartResult::Started {
            assert!(rt.state().allowed[0].calibrating);
            let cal = collect(
                &rt.shared,
                rt.source.as_ref(),
                id,
                Duration::from_millis(600),
                Duration::from_millis(1500),
                Duration::from_millis(60),
            );
            finish(&rt.shared, id, &cal);
        }
        r
    }

    #[test]
    fn calibration_at_one_meter_changes_distance() {
        use crate::calibrate::StartResult;
        let rt = live_runtime(-49);
        assert_eq!(run_calibration(&rt, "other"), StartResult::NotAllowed);
        assert_eq!(run_calibration(&rt, "mine"), StartResult::Started);
        let s = rt.state();
        let a = &s.allowed[0];
        assert!(a.calibrated && !a.calibrating, "{:?}", a.calibration_message);
        assert!(a.calibration_message.as_deref().unwrap().starts_with("已校正"));
        assert!((a.distance_m.unwrap() - 1.0).abs() < 0.01);

        rt.reset_calibration("mine");
        let s = rt.state();
        assert!(!s.allowed[0].calibrated);
        assert!(s.allowed[0].calibration_message.is_none());
        // 回到預設參考 -59：-49 ≈ 0.4 公尺
        assert!((s.allowed[0].distance_m.unwrap() - 0.398).abs() < 0.01);
    }

    #[test]
    fn calibration_at_wrong_distance_is_refused() {
        let rt = live_runtime(-76);
        run_calibration(&rt, "mine");
        let a = &rt.state().allowed[0];
        assert!(!a.calibrated);
        assert!(a.calibration_message.as_deref().unwrap().contains("太弱"), "{:?}", a.calibration_message);
    }

    #[test]
    fn calibration_without_new_readings_fails() {
        // 假來源的裝置期間沒有新廣播（唯一的樣本在按下之前）
        let rt = runtime();
        run_calibration(&rt, "mine");
        let a = &rt.state().allowed[0];
        assert!(!a.calibrated);
        assert!(a.calibration_message.as_deref().unwrap().contains("沒有收到"), "{:?}", a.calibration_message);
    }

    #[test]
    fn reset_during_calibration_cancels_it() {
        use crate::calibrate::{begin, collect, finish};
        let rt = live_runtime(-49);
        begin(&rt.shared, rt.source.as_ref(), "mine");
        rt.reset_calibration("mine"); // 收集前就取消：collect 立刻結束，finish 不套用
        let cal = collect(
            &rt.shared,
            rt.source.as_ref(),
            "mine",
            Duration::from_millis(600),
            Duration::from_millis(1500),
            Duration::from_millis(60),
        );
        finish(&rt.shared, "mine", &cal);
        assert!(!rt.state().allowed[0].calibrated);
    }

    #[test]
    fn unseen_device_cannot_be_calibrated() {
        use crate::calibrate::StartResult;
        let rt = runtime();
        rt.add_device("gone", "Old Phone");
        assert_eq!(run_calibration(&rt, "gone"), StartResult::NotSeen);
        rt.remove_device("gone");
    }

}
