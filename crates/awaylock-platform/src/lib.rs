//! 平台層：實作 `awaylock-core::ports` 中的 trait。
//!
//! | 能力 | macOS | Linux |
//! |---|---|---|
//! | 藍牙掃描 | btleplug（CoreBluetooth） | btleplug（BlueZ） |
//! | 鎖定螢幕 | `pmset displaysleepnow` | `loginctl lock-session` |
//! | 鎖定狀態 | `CGSessionCopyCurrentDictionary` | `loginctl show-session -p LockedHint` |
//! | 開機啟動 | LaunchAgent | XDG autostart |

mod autostart;
mod ble;
mod locker;
mod lockstate;

pub use autostart::SystemAutostart;
pub use ble::BleScanner;
pub use locker::SystemLocker;
pub use lockstate::SystemLockProbe;
