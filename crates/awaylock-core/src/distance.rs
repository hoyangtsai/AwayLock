//! 由藍牙訊號強度（RSSI）粗估距離。
//!
//! 使用對數距離路徑損耗模型：假設 1 公尺處約 -59 dBm、環境係數 2.5。
//! 只是估算，牆壁、人體、手機擺放位置都會造成很大的誤差。

const RSSI_AT_1M: f64 = -59.0;
const PATH_LOSS_EXPONENT: f64 = 2.5;

/// 由 RSSI（dBm）估算距離（公尺）。
pub fn estimated_meters(rssi: i32) -> f64 {
    10f64.powf((RSSI_AT_1M - f64::from(rssi)) / (10.0 * PATH_LOSS_EXPONENT))
}

/// 由距離（公尺）換算 RSSI 門檻（dBm）。距離至少以 0.1 公尺計。
pub fn rssi_threshold(meters: f64) -> i32 {
    (RSSI_AT_1M - 10.0 * PATH_LOSS_EXPONENT * meters.max(0.1).log10()).round() as i32
}

/// 顯示用的距離字串，例如「2.5 公尺」「12 公尺」。
pub fn format_meters(m: f64) -> String {
    if m < 10.0 {
        format!("{m:.1} 公尺")
    } else {
        format!("{m:.0} 公尺")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_meter_is_reference_rssi() {
        assert!((estimated_meters(-59) - 1.0).abs() < 1e-9);
        assert_eq!(rssi_threshold(1.0), -59);
    }

    #[test]
    fn weaker_signal_is_farther() {
        assert!(estimated_meters(-80) > estimated_meters(-60));
    }

    #[test]
    fn threshold_round_trips_roughly() {
        for m in [0.5, 1.0, 2.5, 5.0] {
            let back = estimated_meters(rssi_threshold(m));
            assert!((back - m).abs() / m < 0.15, "{m} -> {back}");
        }
    }

    #[test]
    fn five_meters_matches_swift_default() {
        // Swift 版預設 5 公尺 ≈ -76 dBm
        assert_eq!(rssi_threshold(5.0), -76);
    }

    #[test]
    fn formats() {
        assert_eq!(format_meters(2.5), "2.5 公尺");
        assert_eq!(format_meters(12.0), "12 公尺");
    }
}
