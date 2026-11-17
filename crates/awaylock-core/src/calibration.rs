//! 「在 1 公尺校正」：在一段時間內收集裝置的訊號，取中位數當成該裝置 1 公尺處的 RSSI。
//!
//! 單次按下只看當下的中位數（約 2 秒的廣播）太容易受衰落影響，所以收集數秒、
//! 每次採用新的讀數，最後再取一次中位數；並檢查結果是否像是 1 公尺處的值，
//! 避免在錯誤的距離校正後，鎖定門檻變得永遠不會觸發或太容易觸發。

use crate::device::NearbyDevice;
use std::time::{Duration, Instant};

/// 收集讀數的時間。
pub const CALIBRATION_DURATION: Duration = Duration::from_secs(6);
/// 至少要有幾筆新讀數才算數。
pub const MIN_READINGS: usize = 8;
/// 1 公尺處 RSSI 的合理範圍（dBm）。超出範圍多半是裝置不在 1 公尺處。
pub const MIN_RSSI_AT_1M: i32 = -70;
pub const MAX_RSSI_AT_1M: i32 = -35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalibrationError {
    /// 收到的新讀數不夠（訊號斷斷續續或裝置太遠）。
    NotEnoughReadings { got: usize },
    /// 量到的值不像 1 公尺處的訊號。
    OutOfRange { rssi: i32 },
}

/// 收集校正讀數。
#[derive(Debug, Default)]
pub struct Calibration {
    readings: Vec<i32>,
    last: Option<Instant>,
}

impl Calibration {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取一次裝置的讀數；只有出現新樣本時才記錄，同一個中位數不會重複計入。
    pub fn observe(&mut self, device: &NearbyDevice, now: Instant) {
        if let Some((at, rssi)) = device.calibration_reading(now) {
            if self.last != Some(at) {
                self.last = Some(at);
                self.readings.push(rssi);
            }
        }
    }

    pub fn readings(&self) -> usize {
        self.readings.len()
    }

    /// 結束收集，回傳 1 公尺處的 RSSI（dBm）。
    pub fn finish(&self) -> Result<f64, CalibrationError> {
        if self.readings.len() < MIN_READINGS {
            return Err(CalibrationError::NotEnoughReadings { got: self.readings.len() });
        }
        let mut v = self.readings.clone();
        v.sort_unstable();
        let n = v.len();
        let m = if n % 2 == 1 { f64::from(v[n / 2]) } else { f64::from(v[n / 2 - 1] + v[n / 2]) / 2.0 };
        let rounded = m.round() as i32;
        if !(MIN_RSSI_AT_1M..=MAX_RSSI_AT_1M).contains(&rounded) {
            return Err(CalibrationError::OutOfRange { rssi: rounded });
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::RSSI_WINDOW;

    /// 模擬每 300 ms 一次廣播，回傳最後時間。
    fn feed(cal: &mut Calibration, rssis: &[i32]) {
        let mut t = Instant::now();
        let mut d = NearbyDevice::new("x".into(), None, rssis[0], false, t);
        for (i, &r) in rssis.iter().enumerate().skip(1) {
            t += Duration::from_millis(300);
            d.update(r, None, false, t);
            if i + 1 >= RSSI_WINDOW {
                cal.observe(&d, t);
                cal.observe(&d, t); // 同一個樣本重複取，不應重複計入
            }
        }
    }

    #[test]
    fn steady_signal_gives_its_value() {
        let mut cal = Calibration::new();
        feed(&mut cal, &[-50; 20]);
        assert_eq!(cal.readings(), 14);
        assert_eq!(cal.finish(), Ok(-50.0));
    }

    #[test]
    fn fades_do_not_drag_the_result() {
        let mut cal = Calibration::new();
        let mut rssis = vec![-50; 24];
        for i in (3..24).step_by(4) {
            rssis[i] = -75; // 偶發的深度衰落
        }
        feed(&mut cal, &rssis);
        assert_eq!(cal.finish(), Ok(-50.0));
    }

    #[test]
    fn too_far_is_rejected() {
        // 在 5 公尺處按下：-76 不像 1 公尺的值，存下去會讓門檻變成 -93，永遠不會鎖定
        let mut cal = Calibration::new();
        feed(&mut cal, &[-76; 20]);
        assert_eq!(cal.finish(), Err(CalibrationError::OutOfRange { rssi: -76 }));
    }

    #[test]
    fn touching_the_mac_is_rejected() {
        let mut cal = Calibration::new();
        feed(&mut cal, &[-30; 20]);
        assert_eq!(cal.finish(), Err(CalibrationError::OutOfRange { rssi: -30 }));
    }

    #[test]
    fn too_few_readings_is_rejected() {
        let mut cal = Calibration::new();
        feed(&mut cal, &[-50; 10]);
        assert_eq!(cal.finish(), Err(CalibrationError::NotEnoughReadings { got: 4 }));
    }
}
