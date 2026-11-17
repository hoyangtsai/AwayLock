//! 由藍牙訊號強度（RSSI）粗估距離。
//!
//! 使用對數距離路徑損耗模型：距離 = 10^((1 公尺處 RSSI − RSSI) / (10 × 環境係數))，環境係數 2.5。
//! 1 公尺處的 RSSI 依裝置發射功率而不同（見 [`reference_rssi`]）。
//! 只是估算，牆壁、人體、手機擺放位置都會造成很大的誤差。

/// 一般裝置在 1 公尺處的 RSSI（dBm），iBeacon 常用的校準值。
pub const RSSI_AT_1M: f64 = -59.0;

/// iPhone 在 1 公尺處的 RSSI（dBm）。iPhone 的發射功率比一般 BLE 裝置高約 9 dB：
/// 實測 iPhone 17 Pro 在 3 公尺處中位數約 -60～-62 dBm，套用 -59 會被估成 1.1～1.3 公尺。
pub const PHONE_RSSI_AT_1M: f64 = -50.0;

pub const PATH_LOSS_EXPONENT: f64 = 2.5;

/// 依裝置名稱決定 1 公尺處的 RSSI（dBm）。名稱含「iPhone」的用 [`PHONE_RSSI_AT_1M`]，其餘用 [`RSSI_AT_1M`]。
pub fn reference_rssi(name: Option<&str>) -> f64 {
    match name {
        Some(n) if n.to_lowercase().contains("iphone") => PHONE_RSSI_AT_1M,
        _ => RSSI_AT_1M,
    }
}

/// 由 RSSI（dBm）估算距離（公尺），使用一般裝置的 1 公尺 RSSI。
pub fn estimated_meters(rssi: i32) -> f64 {
    estimated_meters_with(rssi, RSSI_AT_1M)
}

/// 由 RSSI（dBm）與該裝置 1 公尺處的 RSSI 估算距離（公尺）。
pub fn estimated_meters_with(rssi: i32, rssi_at_1m: f64) -> f64 {
    10f64.powf((rssi_at_1m - f64::from(rssi)) / (10.0 * PATH_LOSS_EXPONENT))
}

/// 由距離（公尺）換算 RSSI 門檻（dBm），使用一般裝置的 1 公尺 RSSI。距離至少以 0.1 公尺計。
pub fn rssi_threshold(meters: f64) -> i32 {
    rssi_threshold_with(meters, RSSI_AT_1M)
}

/// 由距離（公尺）與該裝置 1 公尺處的 RSSI 換算 RSSI 門檻（dBm）。距離至少以 0.1 公尺計。
pub fn rssi_threshold_with(meters: f64, rssi_at_1m: f64) -> i32 {
    (rssi_at_1m - 10.0 * PATH_LOSS_EXPONENT * meters.max(0.1).log10()).round() as i32
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
    fn iphone_uses_stronger_reference() {
        assert_eq!(reference_rssi(Some("Hoyang’s iPhone 17 Pro")), PHONE_RSSI_AT_1M);
        assert_eq!(reference_rssi(Some("IPHONE")), PHONE_RSSI_AT_1M);
        assert_eq!(reference_rssi(Some("Hoyang’s Apple Watch")), RSSI_AT_1M);
        assert_eq!(reference_rssi(Some("Pixel 9")), RSSI_AT_1M);
        assert_eq!(reference_rssi(None), RSSI_AT_1M);
    }

    #[test]
    fn iphone_at_three_meters_reads_about_three_meters() {
        // 實測：iPhone 17 Pro 在 3 公尺處中位數 -60～-62 dBm
        let near = estimated_meters_with(-60, PHONE_RSSI_AT_1M);
        let far = estimated_meters_with(-62, PHONE_RSSI_AT_1M);
        assert!((2.4..=2.6).contains(&near), "{near}");
        assert!((2.9..=3.1).contains(&far), "{far}");
    }

    #[test]
    fn threshold_with_reference_shifts_by_offset() {
        for m in [1.0, 2.5, 5.0] {
            assert_eq!(rssi_threshold_with(m, PHONE_RSSI_AT_1M), rssi_threshold(m) + 9);
        }
    }

    #[test]
    fn formats() {
        assert_eq!(format_meters(2.5), "2.5 公尺");
        assert_eq!(format_meters(12.0), "12 公尺");
    }
}
