//! 「埠」：核心層需要平台提供的能力。由 `awaylock-platform` 依作業系統實作。

use crate::device::NearbyDevice;
use std::collections::HashMap;

/// 附近藍牙裝置的來源。
pub trait DeviceSource: Send + Sync {
    /// 藍牙是否可用（已開啟且已授權）。
    fn bluetooth_ready(&self) -> bool;
    /// 給使用者看的狀態文字，例如「掃描中」「藍牙已關閉」。
    fn status_text(&self) -> String;
    /// 目前掃描到的裝置（id → 裝置）。
    fn snapshot(&self) -> HashMap<String, NearbyDevice>;
}

/// 鎖定螢幕。
pub trait ScreenLocker: Send + Sync {
    fn lock(&self) -> std::io::Result<()>;
}

/// 查詢電腦目前是否處於鎖定狀態。
pub trait LockStateProbe: Send + Sync {
    fn is_locked(&self) -> bool;
}

/// 開機（登入）時自動啟動。
pub trait Autostart: Send + Sync {
    fn is_enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool) -> Result<(), String>;
}
