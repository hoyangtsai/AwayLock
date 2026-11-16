//! 給介面使用的狀態檢視（可序列化成 JSON）。
//!
//! 一般狀態（[`StateView`]）只含公尺距離，不含 dBm。
//! 除錯狀態（[`DebugView`]）只存在 debug 編譯；發布版沒有這些程式碼。

use crate::Runtime;
use awaylock_core::distance::estimated_meters;
use awaylock_core::NearbyDevice;
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct AllowedView {
    pub id: String,
    pub name: String,
    /// 最近 15 秒內有收到廣播。
    pub seen: bool,
    pub distance_m: Option<f64>,
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
    pub allowed: Vec<AllowedView>,
    /// 附近裝置（已在允許列表的不含），距離由近到遠。
    pub nearby: Vec<NearbyView>,
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
                    distance_m: seen_dev.map(|d| estimated_meters(d.rssi)),
                }
            })
            .collect();

        let mut others: Vec<&NearbyDevice> =
            devices.values().filter(|d| !st.allowlist.contains(&d.id)).collect();
        others.sort_by(|a, b| b.rssi.cmp(&a.rssi));
        let nearby = others
            .into_iter()
            .map(|d| NearbyView {
                id: d.id.clone(),
                name: d.display_name(),
                is_apple: d.is_apple,
                distance_m: estimated_meters(d.rssi),
            })
            .collect();

        StateView {
            distance_m: st.settings.distance_m,
            bluetooth_status: self.source.status_text(),
            bluetooth_ready: self.source.bluetooth_ready(),
            allowed,
            nearby,
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
    use awaylock_core::distance::estimated_meters;
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
                    distance_m: d.map(|d| estimated_meters(d.rssi)),
                    age_secs: d.map(|d| now.saturating_duration_since(d.last_seen).as_secs()),
                }
            })
            .collect();

        let mut all: Vec<&NearbyDevice> = devices.values().collect();
        all.sort_by(|a, b| b.rssi.cmp(&a.rssi));
        let all = all
            .into_iter()
            .map(|d| DebugDevice {
                id: d.id.clone(),
                name: d.display_name(),
                is_apple: d.is_apple,
                allowed: st.allowlist.contains(&d.id),
                rssi: d.rssi,
                distance_m: estimated_meters(d.rssi),
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
        // 所有測試都用暫存資料夾，絕不寫入使用者的真實設定
        std::env::set_var("AWAYLOCK_CONFIG_DIR", std::env::temp_dir().join("awaylock-test-config"));
        let now = Instant::now();
        let mut devs = HashMap::new();
        devs.insert("mine".into(), NearbyDevice::new("mine".into(), Some("My Watch".into()), -59, true, now));
        devs.insert("other".into(), NearbyDevice::new("other".into(), None, -70, false, now));
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
        // 已允許的不出現在「附近」，另一個才出現
        assert_eq!(s.nearby.len(), 1);
        assert_eq!(s.nearby[0].id, "other");
    }

    #[test]
    fn mutations_update_state() {
        let rt = runtime();
        rt.set_distance(2.3);
        assert_eq!(rt.state().distance_m, 2.5);
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
}
