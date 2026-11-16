//! 以 btleplug 持續掃描附近的 BLE 廣播（macOS 與 Linux 共用）。

use awaylock_core::device::{continuity_types, NearbyDevice, APPLE_COMPANY_ID};
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
                match s.devices.get_mut(&key) {
                    Some(d) => {
                        d.update(rssi, props.local_name, is_apple, now);
                        d.merge_apple_types(&apple_types);
                    }
                    None => {
                        let mut d = NearbyDevice::new(key.clone(), props.local_name, rssi, is_apple, now);
                        d.merge_apple_types(&apple_types);
                        s.devices.insert(key, d);
                    }
                }
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
