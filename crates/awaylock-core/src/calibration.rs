//! 「在 1 公尺校正」：收集裝置在一段時間內的每一次廣播，取中位數當成該裝置 1 公尺處的 RSSI。
//!
//! 直接收集原始樣本（同一次廣播的重複回報已合併），不依賴距離用的中位數視窗：
//! iPhone 鎖定時約 2 秒才廣播一次，視窗在樣本有效期限內根本填不滿。
//! 結果還要像是 1 公尺處的值，避免在錯誤的距離校正後，鎖定門檻永遠不會觸發或太容易觸發。

use crate::device::NearbyDevice;
use std::time::{Duration, Instant};

/// 至少收集多久（即使樣本已經夠了，也多收一點來平均掉衰落）。
pub const MIN_CALIBRATION_TIME: Duration = Duration::from_secs(6);
/// 最多收集多久；廣播稀疏的裝置會一直收到這個時間。
pub const MAX_CALIBRATION_TIME: Duration = Duration::from_secs(30);
/// 至少要有幾次廣播才算數。
pub const MIN_READINGS: usize = 6;
/// 1 公尺處 RSSI 的合理範圍（dBm）。超出範圍多半是裝置不在 1 公尺處。
pub const MIN_RSSI_AT_1M: i32 = -70;
pub const MAX_RSSI_AT_1M: i32 = -35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalibrationError {
    /// 收到的廣播不夠。
    NotEnoughReadings { got: usize },
    /// 量到的值不像 1 公尺處的訊號。
    OutOfRange { rssi: i32 },
}

/// 收集校正樣本。
#[derive(Debug)]
pub struct Calibration {
    started: Instant,
    samples: Vec<i32>,
    last: Option<Instant>,
}

impl Calibration {
    pub fn new(started: Instant) -> Self {
        Self { started, samples: Vec::new(), last: None }
    }

    /// 記錄裝置的最新樣本：按下校正之後收到的每一次廣播記一筆。
    /// 同一次廣播稍後又被回報為更強的值時，改寫剛記下的那一筆。
    pub fn observe(&mut self, device: &NearbyDevice) {
        let Some((at, rssi)) = device.latest_sample() else { return };
        if at < self.started {
            return;
        }
        if self.last == Some(at) {
            if let Some(v) = self.samples.last_mut() {
                *v = rssi;
            }
        } else {
            self.last = Some(at);
            self.samples.push(rssi);
        }
    }

    pub fn readings(&self) -> usize {
        self.samples.len()
    }

    /// 是否可以結束收集：收滿最短時間且樣本足夠，或已到最長時間。
    pub fn is_complete(&self, now: Instant, min_time: Duration, max_time: Duration) -> bool {
        let elapsed = now.saturating_duration_since(self.started);
        elapsed >= max_time || (elapsed >= min_time && self.readings() >= MIN_READINGS)
    }

    /// 結束收集，回傳 1 公尺處的 RSSI（dBm）。
    pub fn finish(&self) -> Result<f64, CalibrationError> {
        if self.samples.len() < MIN_READINGS {
            return Err(CalibrationError::NotEnoughReadings { got: self.samples.len() });
        }
        let mut v = self.samples.clone();
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

    /// 模擬每隔 `gap` 一次廣播（每次廣播重複回報兩次），每次都讓校正取樣兩次。
    fn feed(rssis: &[i32], gap: Duration) -> Calibration {
        let t0 = Instant::now();
        let mut cal = Calibration::new(t0);
        let mut d: Option<NearbyDevice> = None;
        for (i, &r) in rssis.iter().enumerate() {
            let t = t0 + gap * (i as u32 + 1);
            match d.as_mut() {
                Some(d) => d.update(r, None, true, t),
                None => d = Some(NearbyDevice::new("x".into(), None, r, true, t)),
            }
            let dev = d.as_mut().unwrap();
            dev.update(r - 3, None, true, t + Duration::from_millis(1)); // 重複回報，較弱
            cal.observe(dev);
            cal.observe(dev);
        }
        cal
    }

    #[test]
    fn steady_signal_gives_its_value() {
        let cal = feed(&[-61; 10], Duration::from_millis(300));
        assert_eq!(cal.readings(), 10);
        assert_eq!(cal.finish(), Ok(-61.0));
    }

    #[test]
    fn slow_advertiser_still_calibrates() {
        // 鎖定的 iPhone：約 2 秒才廣播一次。視窗永遠填不滿，但每次廣播都算一筆
        let cal = feed(&[-61, -64, -60, -61, -65, -58, -61, -62, -61], Duration::from_secs(2));
        assert_eq!(cal.readings(), 9);
        assert_eq!(cal.finish(), Ok(-61.0));
    }

    #[test]
    fn fades_do_not_drag_the_result() {
        let mut rssis = vec![-50; 12];
        for i in (1..12).step_by(4) {
            rssis[i] = -78;
        }
        assert_eq!(feed(&rssis, Duration::from_millis(300)).finish(), Ok(-50.0));
    }

    #[test]
    fn stronger_duplicate_replaces_the_recorded_value() {
        let t0 = Instant::now();
        let mut cal = Calibration::new(t0);
        let mut d = NearbyDevice::new("x".into(), None, -70, true, t0);
        cal.observe(&d);
        d.update(-60, None, true, t0 + Duration::from_millis(1));
        cal.observe(&d);
        assert_eq!(cal.readings(), 1);
        assert_eq!(cal.samples, vec![-60]);
    }

    #[test]
    fn samples_from_before_the_press_are_ignored() {
        let t0 = Instant::now();
        let d = NearbyDevice::new("x".into(), None, -60, true, t0);
        let mut cal = Calibration::new(t0 + Duration::from_millis(500));
        cal.observe(&d);
        assert_eq!(cal.readings(), 0);
    }

    #[test]
    fn completes_after_min_time_with_enough_samples_or_at_max_time() {
        let (min, max) = (Duration::from_secs(6), Duration::from_secs(30));
        let few = feed(&[-61; 3], Duration::from_secs(2));
        let many = feed(&[-61; 10], Duration::from_millis(300));
        let t0 = many.started;
        assert!(!many.is_complete(t0 + Duration::from_secs(5), min, max));
        assert!(many.is_complete(t0 + Duration::from_secs(6), min, max));
        assert!(!few.is_complete(few.started + Duration::from_secs(10), min, max));
        assert!(few.is_complete(few.started + max, min, max));
    }

    #[test]
    fn too_far_is_rejected() {
        // 在 5 公尺外按下：存下去會讓門檻低到永遠不會鎖定
        let cal = feed(&[-80; 10], Duration::from_millis(300));
        assert_eq!(cal.finish(), Err(CalibrationError::OutOfRange { rssi: -80 }));
    }

    #[test]
    fn touching_the_mac_is_rejected() {
        let cal = feed(&[-30; 10], Duration::from_millis(300));
        assert_eq!(cal.finish(), Err(CalibrationError::OutOfRange { rssi: -30 }));
    }

    #[test]
    fn too_few_readings_is_rejected() {
        let cal = feed(&[-61; 4], Duration::from_secs(2));
        assert_eq!(cal.finish(), Err(CalibrationError::NotEnoughReadings { got: 4 }));
    }
}
