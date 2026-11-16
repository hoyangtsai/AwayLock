//! 背景檢查執行緒：定期收集裝置與鎖定狀態，交給核心的 `Monitor` 判定，必要時鎖定螢幕。

use awaylock_core::ports::{DeviceSource, LockStateProbe, ScreenLocker};
use awaylock_core::{Action, AllowList, Monitor, MonitorConfig, Settings, Snapshot};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 檢查間隔。
pub const TICK_INTERVAL: Duration = Duration::from_secs(5);

/// UI 與背景執行緒共用的狀態。
pub struct SharedState {
    pub settings: Settings,
    pub allowlist: AllowList,
    pub monitor: Monitor,
    pub last_check: Option<Instant>,
}

impl SharedState {
    pub fn new(settings: Settings, allowlist: AllowList) -> Self {
        Self { settings, allowlist, monitor: Monitor::new(MonitorConfig::default()), last_check: None }
    }
}

pub type Shared = Arc<Mutex<SharedState>>;

pub fn spawn(
    shared: Shared,
    source: Arc<dyn DeviceSource>,
    locker: Arc<dyn ScreenLocker>,
    probe: Arc<dyn LockStateProbe>,
) {
    std::thread::Builder::new()
        .name("awaylock-engine".into())
        .spawn(move || loop {
            std::thread::sleep(TICK_INTERVAL);

            // 先在鎖外取得平台資料
            let devices = source.snapshot();
            let ready = source.bluetooth_ready();
            let screen_locked = probe.is_locked();
            let now = Instant::now();

            let action = {
                let mut st = shared.lock().unwrap();
                let allowed = st.allowlist.devices().to_vec();
                let threshold = st.settings.threshold();
                st.last_check = Some(now);
                let snap = Snapshot {
                    now,
                    screen_locked,
                    bluetooth_ready: ready,
                    allowed: &allowed,
                    devices: &devices,
                    threshold,
                };
                st.monitor.tick(&snap)
            };

            if let Action::Lock { device_name } = action {
                eprintln!("「{device_name}」離開，鎖定螢幕");
                if let Err(e) = locker.lock() {
                    eprintln!("鎖定失敗：{e}");
                }
            }
        })
        .expect("spawn engine thread");
}
