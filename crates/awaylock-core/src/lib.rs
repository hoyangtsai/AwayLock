//! AwayLock 核心層：純邏輯，不依賴任何作業系統 API。
//!
//! 分層：
//! - `awaylock-core`（本 crate）：距離換算、允許列表、設定、鎖定判定狀態機，
//!   以及「埠」（ports，即 trait）。
//! - `awaylock-platform`：實作這些 trait（藍牙掃描、鎖定螢幕、鎖定狀態偵測、開機啟動），
//!   依平台（macOS / Linux）切換實作。
//! - `awaylock-app`：狀態列圖示、設定視窗，以及把各層接起來。

pub mod allowlist;
pub mod device;
pub mod distance;
pub mod monitor;
pub mod ports;
pub mod settings;

pub use allowlist::{AllowList, AllowedDevice};
pub use device::NearbyDevice;
pub use monitor::{Action, Monitor, MonitorConfig, Phase, Presence, Snapshot};
pub use settings::Settings;
