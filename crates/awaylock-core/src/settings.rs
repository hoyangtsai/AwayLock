//! 使用者設定。

use crate::distance::rssi_threshold;
use serde::{Deserialize, Serialize};

pub const MIN_DISTANCE_M: f64 = 0.5;
pub const MAX_DISTANCE_M: f64 = 5.0;
pub const DISTANCE_STEP_M: f64 = 0.5;
pub const DEFAULT_DISTANCE_M: f64 = 5.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 離開距離（公尺）。範圍 0.5～5，以 0.5 為一格。
    pub distance_m: f64,
    /// 僅開發版（debug 編譯）存在；發布版沒有這個欄位。
    #[cfg(debug_assertions)]
    pub debug_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            distance_m: DEFAULT_DISTANCE_M,
            #[cfg(debug_assertions)]
            debug_mode: false,
        }
    }
}

impl Settings {
    /// 把距離限制在範圍內並對齊 0.5 公尺。
    pub fn normalize_distance(m: f64) -> f64 {
        let snapped = (m / DISTANCE_STEP_M).round() * DISTANCE_STEP_M;
        snapped.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M)
    }

    pub fn set_distance(&mut self, m: f64) {
        self.distance_m = Self::normalize_distance(m);
    }

    /// 修正載入後可能不合法的值。
    pub fn sanitized(mut self) -> Self {
        self.distance_m = Self::normalize_distance(self.distance_m);
        self
    }

    /// 對應的 RSSI 門檻（dBm），內部判定使用。
    pub fn threshold(&self) -> i32 {
        rssi_threshold(self.distance_m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(Settings::normalize_distance(2.3), 2.5);
        assert_eq!(Settings::normalize_distance(0.0), 0.5);
        assert_eq!(Settings::normalize_distance(99.0), 5.0);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.distance_m, DEFAULT_DISTANCE_M);
    }
}
