//! 以 btleplug 持續掃描附近的 BLE 廣播（macOS 與 Linux 共用）。

use awaylock_core::device::{continuity_types, is_valid_rssi, NearbyDevice, APPLE_COMPANY_ID};
use awaylock_core::ports::DeviceSource;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 超過這段時間沒再收到廣播的裝置，從列表移除。
const FORGET_AFTER: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Shared {
    devices: HashMap<String, NearbyDevice>,
    ready: bool,
    status: String,
}

#[derive(Clone)]
pub struct BleScanner {
    shared: Arc<Mutex<Shared>>,
}

impl BleScanner {
    /// 在背景執行緒開始掃描。
    pub fn start() -> Self {
        let shared = Arc::new(Mutex::new(Shared { status: "初始化中…".into(), ..Default::default() }));
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("awaylock-ble".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                rt.block_on(async move {
                    // 掃描失敗（例如藍牙被關閉後重新開啟）時，隔幾秒重試
                    loop {
                        if let Err(msg) = scan(&worker).await {
                            set_status(&worker, false, msg);
                        }
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                });
            })
            .expect("spawn ble thread");
        Self { shared }
    }
}

fn set_status(shared: &Arc<Mutex<Shared>>, ready: bool, text: impl Into<String>) {
    let mut s = shared.lock().unwrap();
    s.ready = ready;
    s.status = text.into();
}

async fn scan(shared: &Arc<Mutex<Shared>>) -> Result<(), String> {
    let manager = Manager::new()
        .await
        .map_err(|e| format!("無法使用藍牙（請確認已授權）：{e}"))?;
    let adapters = manager.adapters().await.map_err(|e| format!("無法取得藍牙介面：{e}"))?;
    let central = adapters.into_iter().next().ok_or_else(|| "找不到藍牙介面或藍牙已關閉".to_string())?;

    let mut events = central.events().await.map_err(|e| format!("無法監聽藍牙事件：{e}"))?;
    central
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| format!("無法開始掃描：{e}"))?;
    set_status(shared, true, "掃描中");

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => {
                let Ok(p) = central.peripheral(&id).await else { continue };
                let Ok(Some(props)) = p.properties().await else { continue };
                let Some(rssi) = props.rssi else { continue };
                let apple_data = props.manufacturer_data.get(&APPLE_COMPANY_ID);
                let is_apple = apple_data.is_some();
                let apple_types = apple_data.map(|d| continuity_types(d)).unwrap_or_default();
                let key = id.to_string();
                let now = Instant::now();
                let rssi = i32::from(rssi);

                let mut s = shared.lock().unwrap();
                apply_advertisement(&mut s.devices, key, props.local_name, rssi, is_apple, &apple_types, now);
            }
            CentralEvent::StateUpdate(state) => {
                // 藍牙關閉時結束，由外層重試
                use btleplug::api::CentralState;
                if matches!(state, CentralState::PoweredOff) {
                    return Err("藍牙已關閉".into());
                }
            }
            _ => {}
        }
    }
    Err("藍牙掃描中斷".into())
}

/// 把一筆廣播套用到裝置表。
///
/// - 已知裝置：交給 `NearbyDevice::update`；RSSI 無效（例如 127）時它只會更新 `last_seen` 等資訊。
/// - 新裝置：必須有有效 RSSI 才建立，避免以無效值當成起始距離。
fn apply_advertisement(
    devices: &mut HashMap<String, NearbyDevice>,
    key: String,
    name: Option<String>,
    rssi: i32,
    is_apple: bool,
    apple_types: &[u8],
    now: Instant,
) {
    match devices.get_mut(&key) {
        Some(d) => {
            d.update(rssi, name, is_apple, now);
            d.merge_apple_types(apple_types);
        }
        None if is_valid_rssi(rssi) => {
            let mut d = NearbyDevice::new(key.clone(), name, rssi, is_apple, now);
            d.merge_apple_types(apple_types);
            devices.insert(key, d);
        }
        None => {}
    }
}

impl DeviceSource for BleScanner {
    fn bluetooth_ready(&self) -> bool {
        self.shared.lock().unwrap().ready
    }

    fn status_text(&self) -> String {
        self.shared.lock().unwrap().status.clone()
    }

    fn snapshot(&self) -> HashMap<String, NearbyDevice> {
        let mut s = self.shared.lock().unwrap();
        let cutoff = Instant::now().checked_sub(FORGET_AFTER);
        if let Some(cutoff) = cutoff {
            s.devices.retain(|_, d| d.last_seen > cutoff);
        }
        s.devices.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adv(m: &mut HashMap<String, NearbyDevice>, rssi: i32, now: Instant) {
        apply_advertisement(m, "d".into(), Some("Watch".into()), rssi, true, &[0x10], now);
    }

    #[test]
    fn new_device_with_invalid_rssi_is_not_created() {
        let mut m = HashMap::new();
        let now = Instant::now();
        adv(&mut m, 127, now);
        adv(&mut m, 0, now);
        assert!(m.is_empty());
    }

    #[test]
    fn new_device_with_valid_rssi_is_created() {
        let mut m = HashMap::new();
        let now = Instant::now();
        adv(&mut m, -60, now);
        let d = &m["d"];
        assert_eq!(d.rssi, -60);
        assert_eq!(d.name.as_deref(), Some("Watch"));
        assert!(d.is_apple);
        assert_eq!(d.apple_types, vec![0x10]);
    }

    #[test]
    fn invalid_rssi_only_refreshes_last_seen() {
        let mut m = HashMap::new();
        let t0 = Instant::now();
        adv(&mut m, -60, t0);
        let dist = m["d"].meters();

        let t1 = t0 + Duration::from_secs(5);
        adv(&mut m, 127, t1);
        let d = &m["d"];
        assert_eq!(d.last_seen, t1, "127 仍代表看到了裝置");
        assert_eq!(d.rssi, -60, "訊號不應被 127 影響");
        assert_eq!(d.meters(), dist, "距離不應被 127 影響");
    }

    #[test]
    fn valid_rssi_updates_existing_device() {
        let mut m = HashMap::new();
        let t0 = Instant::now();
        adv(&mut m, -60, t0);
        adv(&mut m, -70, t0 + Duration::from_secs(1));
        assert_eq!(m.len(), 1);
        assert_eq!(m["d"].rssi, -65); // 兩筆的中位數
    }
}
