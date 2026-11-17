//! 掃描到的藍牙裝置。

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::distance::{estimated_meters_with, reference_rssi, PATH_LOSS_EXPONENT, RSSI_AT_1M};

/// Apple 的藍牙廠商代碼（Company Identifier）。
pub const APPLE_COMPANY_ID: u16 = 0x004C;

#[derive(Debug, Clone, PartialEq)]
pub struct NearbyDevice {
    /// 平台提供的識別碼（macOS 為 UUID，Linux 為藍牙位址）。
    /// Apple 裝置的識別碼會定期更換。
    pub id: String,
    pub name: Option<String>,
    /// 平滑後的訊號強度（dBm）：最近 [`RSSI_WINDOW`] 筆原始值的中位數。
    pub rssi: i32,
    /// 最近的原始 RSSI（dBm）與收到的時間，最舊的在前；超過 [`SAMPLE_TTL`] 的會被丟棄。
    samples: VecDeque<(Instant, i32)>,
    /// 目前這段連續觀察期間收到的有效樣本數；未滿 [`RSSI_WINDOW`] 前距離不做指數平滑。
    seen: usize,
    /// 平滑後的距離（公尺）：對中位數 RSSI 換算出的距離再做指數平滑。
    distance_m: f64,
    /// 套用最新一筆樣本之前的距離，突發合併改寫最新樣本時用它重算平滑距離。
    distance_before_last: f64,
    /// 此裝置在 1 公尺處的 RSSI（dBm），依名稱決定（見 [`reference_rssi`]）。
    rssi_at_1m: f64,
    pub last_seen: Instant,
    pub is_apple: bool,
    /// Apple 廣播資料（Continuity）中出現過的訊息類型，已排序、不重複。
    pub apple_types: Vec<u8>,
}

/// 裝置大致的種類（只能從廣播內容推測，不一定準確）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// 名稱顯示是手機或手錶（例如 "iPhone"、"Apple Watch"）。
    PhoneOrWatch,
    /// 看起來是耳機、定位標籤、喇叭、電視、鍵盤等配件。
    Other,
    /// 資訊不足，無法判斷。
    Unknown,
}

impl DeviceKind {
    pub fn label(self) -> &'static str {
        match self {
            DeviceKind::PhoneOrWatch => "手機/手錶",
            DeviceKind::Other => "其他",
            DeviceKind::Unknown => "未知",
        }
    }
}

// Apple Continuity 訊息類型（依社群逆向研究整理，非官方文件，可能有誤）。
// 只用來辨認「一定不是手機」的配件。
const APPLE_ACCESSORY: &[u8] = &[
    0x07, // 耳機（AirPods / Beats）近距離配對
    0x12, // Find My（AirTag 等）
    0x09, // AirPlay（Apple TV / HomePod / Mac）
];

const OTHER_NAME_HINTS: &[&str] = &[
    "airpods", "airtag", "macbook", "imac", "mac mini", "mac studio", "ipad", "homepod", "apple tv",
    "beats", "buds", "earbuds", "headphone", "headset", "speaker", "soundbar", "keyboard", "mouse",
    "trackpad", "bravia",
];
const PHONE_NAME_HINTS: &[&str] = &["iphone", "phone", "watch"];

/// 解析 Apple 廣播資料（公司代碼之後的部分）中依序出現的訊息類型。
/// 格式為連續的 type(1) + length(1) + data(length)。
pub fn continuity_types(data: &[u8]) -> Vec<u8> {
    let mut types = Vec::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let (t, len) = (data[i], usize::from(data[i + 1]));
        if !types.contains(&t) {
            types.push(t);
        }
        i += 2 + len;
    }
    types.sort_unstable();
    types
}

/// 依名稱與 Apple 廣播類型推測裝置種類。
pub fn classify(name: Option<&str>, is_apple: bool, apple_types: &[u8]) -> DeviceKind {
    if let Some(n) = name.map(str::to_lowercase).filter(|n| !n.is_empty()) {
        // 先比對「其他」：例如 "headphone" 也含有 "phone"
        if OTHER_NAME_HINTS.iter().any(|h| n.contains(h)) {
            return DeviceKind::Other;
        }
        if PHONE_NAME_HINTS.iter().any(|h| n.contains(h)) {
            return DeviceKind::PhoneOrWatch;
        }
        // 有名稱但不像手機或手錶（例如房間名稱的 HomePod / Apple TV）：
        // 名稱是比廣播代碼更強的線索，不再用代碼補判，否則 Nearby Info 這類
        // 所有 Apple 裝置都會送的代碼會造成誤判。
        return DeviceKind::Unknown;
    }
    // 沒有名稱的 Apple 裝置：只能認出「一定不是手機」的配件。
    // Nearby Info、Handoff 等代碼所有 Apple 裝置（iPad、Mac、HomePod…）都會送，
    // 甚至同一支手機的不同廣播位址也會送，無法據此判斷是手機或手錶，所以不當成手機。
    if is_apple && apple_types.iter().any(|t| APPLE_ACCESSORY.contains(t)) {
        return DeviceKind::Other;
    }
    DeviceKind::Unknown
}

impl NearbyDevice {
    pub fn new(id: String, name: Option<String>, rssi: i32, is_apple: bool, now: Instant) -> Self {
        let mut samples = VecDeque::with_capacity(RSSI_WINDOW);
        samples.push_back((now, rssi));
        let rssi_at_1m = reference_rssi(name.as_deref());
        let distance_m = estimated_meters_with(rssi, rssi_at_1m);
        Self {
            id,
            name,
            rssi,
            samples,
            seen: 1,
            distance_m,
            distance_before_last: distance_m,
            rssi_at_1m,
            last_seen: now,
            is_apple,
            apple_types: Vec::new(),
        }
    }

    /// 平滑後的估計距離（公尺）。
    pub fn meters(&self) -> f64 {
        self.distance_m
    }

    /// 此裝置在 1 公尺處的 RSSI（dBm）。
    pub fn rssi_at_1m(&self) -> f64 {
        self.rssi_at_1m
    }

    /// 套用使用者的 1 公尺校正值（`None` 表示沒有校正）後的距離。
    ///
    /// 換參考值只是把距離乘上固定倍數，指數平滑是線性的，所以直接縮放平滑後的距離即可，
    /// 不必重新累積樣本。
    pub fn meters_calibrated(&self, rssi_at_1m: Option<f64>) -> f64 {
        match rssi_at_1m {
            Some(r) => self.distance_m * 10f64.powf((r - self.rssi_at_1m) / (10.0 * PATH_LOSS_EXPONENT)),
            None => self.distance_m,
        }
    }

    /// 把一般裝置的 RSSI 門檻換算成此裝置的門檻：發射功率較強的裝置門檻也較高，
    /// 讓「離開距離」對每種裝置都代表差不多的實際距離。
    /// 使用者校正過（`Some`）就用校正值，否則用依名稱決定的預設值。
    pub fn threshold_calibrated(&self, base_threshold: i32, rssi_at_1m: Option<f64>) -> i32 {
        let r = rssi_at_1m.unwrap_or(self.rssi_at_1m);
        base_threshold + (r - RSSI_AT_1M).round() as i32
    }

    /// 校正用的讀數：`(最新樣本的時間, 中位數 RSSI)`。
    ///
    /// 只有在視窗裡的 [`RSSI_WINDOW`] 筆樣本都還沒過期、且最新一筆是剛收到的（[`READING_FRESH`] 內）時才回傳，
    /// 避免拿樣本稀少或很久以前的中位數校正（例如只收到無效的 127，`last_seen` 有更新但樣本沒有）。
    pub fn calibration_reading(&self, now: Instant) -> Option<(Instant, i32)> {
        let (&(oldest, _), &(newest, _)) = (self.samples.front()?, self.samples.back()?);
        let full = self.samples.len() == RSSI_WINDOW;
        let fresh = now.saturating_duration_since(newest) <= READING_FRESH;
        let unexpired = now.saturating_duration_since(oldest) <= SAMPLE_TTL;
        (full && fresh && unexpired).then_some((newest, self.rssi))
    }

    /// 記錄新看到的 Apple 廣播類型。
    pub fn merge_apple_types(&mut self, types: &[u8]) {
        for t in types {
            if !self.apple_types.contains(t) {
                self.apple_types.push(*t);
            }
        }
        self.apple_types.sort_unstable();
    }

    pub fn kind(&self) -> DeviceKind {
        classify(self.name.as_deref(), self.is_apple, &self.apple_types)
    }

    /// 收到新的廣播：更新訊號（中位數）、距離（指數平滑）、名稱與時間。
    ///
    /// - 無效的 RSSI（見 [`is_valid_rssi`]）不進入視窗，但仍視為「看到了這個裝置」，
    ///   會更新 `last_seen`、名稱與 Apple 旗標。
    /// - 超過 [`SAMPLE_TTL`] 的舊樣本會先被丟棄；若全都過期，視為重新開始觀察。
    /// - 視窗填滿（[`RSSI_WINDOW`] 筆）之前，距離直接取中位數的換算值，避免被第一筆離群值拖偏。
    /// - [`BURST_WINDOW`] 內收到的多筆讀數視為同一次廣播，只保留最強的一筆。
    /// - 名稱改變而使 1 公尺 RSSI 不同時，距離立即以新的值重算。
    pub fn update(&mut self, rssi: i32, name: Option<String>, is_apple: bool, now: Instant) {
        if let Some(n) = name.filter(|n| !n.is_empty()) {
            self.name = Some(n);
            let reference = reference_rssi(self.name.as_deref());
            if reference != self.rssi_at_1m {
                self.rssi_at_1m = reference;
                self.distance_m = estimated_meters_with(self.rssi, reference);
                self.distance_before_last = self.distance_m;
            }
        }
        if is_valid_rssi(rssi) {
            self.push_sample(rssi, now);
        }
        self.last_seen = now;
        self.is_apple = self.is_apple || is_apple;
    }

    fn push_sample(&mut self, rssi: i32, now: Instant) {
        while let Some(&(t, _)) = self.samples.front() {
            if now.saturating_duration_since(t) > SAMPLE_TTL {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        if self.samples.is_empty() {
            self.seen = 0;
        }
        // 同一次廣播常在 0～2 ms 內被回報兩次以上（不同頻道或事件），合併成一筆，取最強的值：
        // 多路徑衰落造成的是個別頻道的訊號驟降，取最大值能過濾掉。
        if let Some(last) = self.samples.back_mut() {
            if now.saturating_duration_since(last.0) < BURST_WINDOW {
                if rssi <= last.1 {
                    return;
                }
                last.1 = rssi;
                // 最新樣本被改寫：中位數與距離都要重算，距離從套用這筆樣本之前的值重新平滑，
                // 不會因同一次廣播被回報多次而多平滑一步
                self.refresh_distance();
                return;
            }
        }
        if self.samples.len() == RSSI_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back((now, rssi));
        self.seen += 1;
        self.distance_before_last = self.distance_m;
        self.refresh_distance();
    }

    /// 由目前的樣本重算中位數，並以 `distance_before_last` 為起點更新距離。
    fn refresh_distance(&mut self) {
        self.rssi = median(self.samples.iter().map(|&(_, r)| r));
        let target = estimated_meters_with(self.rssi, self.rssi_at_1m);
        self.distance_m = if self.seen < RSSI_WINDOW {
            target
        } else {
            smooth_distance(self.distance_before_last, target)
        };
    }

    pub fn display_name(&self) -> String {
        match self.name.as_deref() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                let short: String = self.id.chars().take(8).collect();
                let prefix = if self.is_apple { "Apple 裝置" } else { "未命名裝置" };
                format!("{prefix} {short}")
            }
        }
    }
}

/// RSSI 中位數視窗大小（最近幾筆）。
pub const RSSI_WINDOW: usize = 7;

/// 這段時間內收到的讀數視為同一次廣播。iPhone 約每 270 ms、Apple Watch 約每 185 ms 廣播一次，
/// 重複回報則在 0～2 ms 內，所以 50 ms 能分開兩者。
pub const BURST_WINDOW: Duration = Duration::from_millis(50);

/// 樣本的有效期限：超過這段時間的 RSSI 不再參與中位數。
pub const SAMPLE_TTL: Duration = Duration::from_secs(10);

/// 校正時，最新樣本最多可以是多久以前收到的。
pub const READING_FRESH: Duration = Duration::from_secs(1);

/// 距離指數平滑係數：新值佔 15%。
pub const DISTANCE_ALPHA: f64 = 0.15;

/// RSSI 是否為有效讀數。CoreBluetooth 讀不到訊號時回報 127，
/// 實際訊號強度必為負值（dBm），所以 ≥ 0 的值一律忽略。
pub fn is_valid_rssi(rssi: i32) -> bool {
    rssi < 0
}

/// 取中位數；筆數為偶數時取中間兩筆平均（四捨五入）。空集合回傳 0。
pub fn median(samples: impl IntoIterator<Item = i32>) -> i32 {
    let mut buf = [0i32; RSSI_WINDOW];
    let mut n = 0;
    for r in samples.into_iter().take(RSSI_WINDOW) {
        buf[n] = r;
        n += 1;
    }
    if n == 0 {
        return 0;
    }
    let v = &mut buf[..n];
    v.sort_unstable();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (f64::from(v[n / 2 - 1] + v[n / 2]) / 2.0).round() as i32
    }
}

/// 距離的指數平滑：舊值 85%、新值 15%。
pub fn smooth_distance(old: f64, new: f64) -> f64 {
    old * (1.0 - DISTANCE_ALPHA) + new * DISTANCE_ALPHA
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distance::{estimated_meters, PHONE_RSSI_AT_1M};

    fn dev(rssi: i32) -> NearbyDevice {
        NearbyDevice::new("a".into(), None, rssi, false, Instant::now())
    }

    /// 測試用時鐘：每次前進一個廣播間隔（300 ms），避免連續讀數被當成同一次廣播合併。
    struct Clock(Instant);
    impl Clock {
        fn new() -> Self {
            Clock(Instant::now())
        }
        fn next(&mut self) -> Instant {
            self.0 += Duration::from_millis(300);
            self.0
        }
    }

    #[test]
    fn rssi_127_is_invalid() {
        assert!(!is_valid_rssi(127));
        assert!(!is_valid_rssi(0));
        assert!(is_valid_rssi(-1));
        assert!(is_valid_rssi(-90));
    }

    #[test]
    fn median_odd_and_even() {
        assert_eq!(median([-70, -60, -90]), -70);
        assert_eq!(median([-60, -70]), -65);
        assert_eq!(median([]), 0);
    }

    #[test]
    fn median_ignores_single_spike() {
        let mut clk = Clock::new();
        let mut d = dev(-70);
        for r in [-71, -69, -70, -40, -72, -70] {
            d.update(r, None, false, clk.next());
        }
        // 7 筆：-70 -71 -69 -70 -40 -72 -70，-40 是突波
        assert_eq!(d.rssi, -70);
    }

    #[test]
    fn window_keeps_only_last_seven() {
        let mut clk = Clock::new();
        let mut d = dev(-90);
        for _ in 0..RSSI_WINDOW {
            d.update(-60, None, false, clk.next());
        }
        // 舊的 -90 已被擠出視窗
        assert_eq!(d.samples.len(), RSSI_WINDOW);
        assert_eq!(d.rssi, -60);
    }

    #[test]
    fn outlier_first_sample_does_not_skew_distance() {
        // 第一筆 -45（約 0.3 m）是離群值，其餘都在 -72 附近（約 4 m）
        let mut clk = Clock::new();
        let mut d = dev(-45);
        for r in [-72, -71, -73, -72, -70, -72] {
            d.update(r, None, false, clk.next());
        }
        assert_eq!(d.rssi, -72);
        // 視窗剛填滿：距離直接是中位數換算值，不是被 0.3 m 拖著慢慢爬
        assert!((d.meters() - estimated_meters(-72)).abs() < 1e-9, "{}", d.meters());
    }

    #[test]
    fn distance_smooths_only_after_window_full() {
        let mut clk = Clock::new();
        let mut d = dev(-59);
        for _ in 0..(RSSI_WINDOW - 2) {
            d.update(-59, None, false, clk.next());
        }
        // 已有 RSSI_WINDOW-1 筆，下一筆（第 RSSI_WINDOW 筆）起才平滑
        let before = d.meters();
        d.update(-59, None, false, clk.next());
        assert!((d.meters() - before).abs() < 1e-9);
    }

    #[test]
    fn old_samples_expire() {
        let t0 = Instant::now();
        let mut d = NearbyDevice::new("a".into(), None, -90, false, t0);
        d.update(-90, None, false, t0);
        d.update(-90, None, false, t0);
        // 之後每 4 秒收到一筆 -60：第 3 筆（t0+12）時，t0 的三筆 -90 都已超過 10 秒
        for i in 1..=3u64 {
            d.update(-60, None, false, t0 + Duration::from_secs(4 * i));
        }
        assert_eq!(d.rssi, -60);
        assert!(d.samples.iter().all(|&(t, _)| t > t0));
    }

    #[test]
    fn sparse_advertiser_follows_signal_within_ttl() {
        // 每 5 秒一筆：不會被 10 秒前以外的舊資料拖住
        let t0 = Instant::now();
        let mut d = NearbyDevice::new("a".into(), None, -60, false, t0);
        for i in 1..=4u64 {
            d.update(-60, None, false, t0 + Duration::from_secs(5 * i));
        }
        for i in 5..=7u64 {
            d.update(-85, None, false, t0 + Duration::from_secs(5 * i));
        }
        // t=35：t≤20 的 -60 都已超過 10 秒而過期，視窗只剩 t=25、30、35 的 -85
        assert_eq!(d.rssi, -85);
    }

    #[test]
    fn gap_longer_than_ttl_restarts_observation() {
        let t0 = Instant::now();
        let mut d = NearbyDevice::new("a".into(), None, -60, false, t0);
        for _ in 0..10 {
            d.update(-60, None, false, t0);
        }
        // 隔了很久再出現在遠處：不應該被舊的距離平滑拖住
        d.update(-85, None, false, t0 + Duration::from_secs(60));
        assert_eq!(d.rssi, -85);
        assert!((d.meters() - estimated_meters(-85)).abs() < 1e-9);
    }

    #[test]
    fn invalid_rssi_refreshes_last_seen_but_not_samples() {
        let t0 = Instant::now();
        let mut d = NearbyDevice::new("a".into(), Some("Watch".into()), -60, false, t0);
        let t1 = t0 + Duration::from_secs(3);
        d.update(127, None, true, t1);
        assert_eq!(d.last_seen, t1);
        assert!(d.is_apple);
        assert_eq!(d.samples.len(), 1);
        assert_eq!(d.rssi, -60);
        assert!((d.meters() - estimated_meters(-60)).abs() < 1e-9);
    }

    #[test]
    fn burst_readings_merge_into_one_sample() {
        let t0 = Instant::now();
        let mut d = NearbyDevice::new("a".into(), None, -70, false, t0);
        d.update(-70, None, false, t0 + Duration::from_millis(1)); // 重複回報
        assert_eq!(d.samples.len(), 1);
        d.update(-65, None, false, t0 + Duration::from_millis(2)); // 同一次廣播較強的頻道
        assert_eq!(d.samples.len(), 1);
        assert_eq!(d.rssi, -65);
        d.update(-80, None, false, t0 + Duration::from_millis(3)); // 同一次廣播的衰落頻道
        assert_eq!(d.rssi, -65);
        assert_eq!(d.last_seen, t0 + Duration::from_millis(3));
        d.update(-72, None, false, t0 + Duration::from_millis(300)); // 下一次廣播
        assert_eq!(d.samples.len(), 2);
    }

    #[test]
    fn iphone_distance_uses_phone_reference() {
        let d = NearbyDevice::new("a".into(), Some("Hoyang’s iPhone 17 Pro".into()), -62, true, Instant::now());
        assert_eq!(d.rssi_at_1m(), PHONE_RSSI_AT_1M);
        assert!((d.meters() - 3.0).abs() < 0.05, "{}", d.meters());
        // 其他裝置維持一般的 1 公尺 RSSI
        assert!((dev(-62).meters() - estimated_meters(-62)).abs() < 1e-9);
    }

    #[test]
    fn name_arriving_later_switches_reference_immediately() {
        let mut clk = Clock::new();
        let mut d = dev(-62);
        for _ in 0..RSSI_WINDOW {
            d.update(-62, None, false, clk.next());
        }
        assert!((d.meters() - estimated_meters(-62)).abs() < 1e-9); // 約 1.3 m
        // 名稱稍後才出現：距離直接改用 iPhone 的換算，不必等指數平滑慢慢爬到 3 m
        d.update(-62, Some("Hoyang’s iPhone 17 Pro".into()), true, clk.next());
        assert!((d.meters() - 3.0).abs() < 0.05, "{}", d.meters());
    }

    #[test]
    fn threshold_follows_device_reference() {
        let phone = NearbyDevice::new("p".into(), Some("My iPhone".into()), -60, true, Instant::now());
        let watch = NearbyDevice::new("w".into(), Some("My Apple Watch".into()), -60, true, Instant::now());
        assert_eq!(phone.threshold_calibrated(-76, None), -67);
        assert_eq!(watch.threshold_calibrated(-76, None), -76);
    }

    #[test]
    fn distance_smoothing_uses_alpha() {
        assert!((smooth_distance(1.0, 3.0) - 1.3).abs() < 1e-9);
        assert!((smooth_distance(2.0, 2.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn new_device_distance_matches_rssi() {
        assert!((dev(-59).meters() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn distance_moves_slowly_toward_new_signal() {
        let mut clk = Clock::new();
        let mut d = dev(-59); // 1 公尺
        for _ in 0..6 {
            d.update(-59, None, false, clk.next()); // 視窗填滿 7 筆 -59
        }
        for _ in 0..3 {
            d.update(-80, None, false, clk.next()); // 4 筆 -59 + 3 筆 -80，中位數仍為 -59
        }
        assert_eq!(d.rssi, -59);
        assert!((d.meters() - 1.0).abs() < 1e-9);
        // 第 4 筆 -80：中位數翻成 -80，距離只往目標走 15%
        d.update(-80, None, false, clk.next());
        assert_eq!(d.rssi, -80);
        let target = estimated_meters(-80);
        let expected = 1.0 * 0.85 + target * 0.15;
        assert!((d.meters() - expected).abs() < 1e-9, "{} vs {expected}", d.meters());
        assert!(d.meters() < target);
    }

    #[test]
    fn jitter_gives_stable_distance() {
        // 模擬 5 步外 RSSI 大幅跳動：顯示距離的範圍應遠小於原始值
        let raw = [-62, -78, -66, -84, -64, -75, -60, -80, -68, -72, -65, -77, -63, -82, -70];
        let mut clk = Clock::new();
        let mut d = dev(raw[0]);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for (i, r) in raw.iter().enumerate().skip(1) {
            d.update(*r, None, false, clk.next());
            if i >= 7 {
                lo = lo.min(d.meters());
                hi = hi.max(d.meters());
            }
        }
        let raw_span = estimated_meters(-84) - estimated_meters(-60);
        assert!(hi - lo < raw_span / 3.0, "span {} vs raw {raw_span}", hi - lo);
    }

    #[test]
    fn update_keeps_known_name() {
        let now = Instant::now();
        let mut d = NearbyDevice::new("abc".into(), Some("Watch".into()), -60, false, now);
        d.update(-62, None, true, now);
        assert_eq!(d.name.as_deref(), Some("Watch"));
        assert!(d.is_apple);
    }

    #[test]
    fn parses_continuity_types() {
        // Nearby Info(0x10, 5 bytes) + Handoff(0x0C, 2 bytes)
        let data = [0x10, 0x05, 1, 2, 3, 4, 5, 0x0C, 0x02, 9, 9];
        assert_eq!(continuity_types(&data), vec![0x0C, 0x10]);
        // 長度超出時不會 panic
        assert_eq!(continuity_types(&[0x10, 0xFF, 1]), vec![0x10]);
        assert!(continuity_types(&[]).is_empty());
    }

    #[test]
    fn classifies_by_name() {
        assert_eq!(classify(Some("Hoyang's iPhone"), false, &[]), DeviceKind::PhoneOrWatch);
        assert_eq!(classify(Some("Galaxy Watch5"), false, &[]), DeviceKind::PhoneOrWatch);
        // "headphone" 含 "phone"，但應判為其他
        assert_eq!(classify(Some("WH-1000XM6 Headphones"), false, &[]), DeviceKind::Other);
        assert_eq!(classify(Some("AirPods Pro"), true, &[0x10]), DeviceKind::Other);
        assert_eq!(classify(None, false, &[]), DeviceKind::Unknown);
    }

    #[test]
    fn named_device_is_judged_by_name_only() {
        // 房間名稱的 HomePod / Apple TV 也會送 Nearby Info，不能因此當成手機
        assert_eq!(classify(Some("客廳"), true, &[0x10]), DeviceKind::Unknown);
        assert_eq!(classify(Some("客廳 (2)"), true, &[0x10, 0x0C]), DeviceKind::Unknown);
        // 名稱是手機或手錶才算
        assert_eq!(classify(Some("Hoyang's iPhone 17 Pro"), true, &[0x10]), DeviceKind::PhoneOrWatch);
        assert_eq!(classify(Some("Hoyang's Apple Watch"), true, &[]), DeviceKind::PhoneOrWatch);
    }

    #[test]
    fn unnamed_apple_device_is_never_assumed_to_be_a_phone() {
        // Nearby Info / Handoff 所有 Apple 裝置都會送，不能據此判定為手機
        assert_eq!(classify(None, true, &[0x10]), DeviceKind::Unknown);
        assert_eq!(classify(None, true, &[0x0B, 0x10]), DeviceKind::Unknown);
        // 耳機、AirTag、AirPlay 裝置可以確定是其他
        assert_eq!(classify(None, true, &[0x07]), DeviceKind::Other);
        assert_eq!(classify(None, true, &[0x12]), DeviceKind::Other);
        assert_eq!(classify(None, true, &[0x09, 0x10]), DeviceKind::Other);
        assert_eq!(classify(None, true, &[0x05]), DeviceKind::Unknown);
    }

    #[test]
    fn display_name_fallbacks() {
        let now = Instant::now();
        let d = NearbyDevice::new("12345678-aaaa".into(), None, -60, true, now);
        assert_eq!(d.display_name(), "Apple 裝置 12345678");
        let d = NearbyDevice::new("12345678-aaaa".into(), None, -60, false, now);
        assert_eq!(d.display_name(), "未命名裝置 12345678");
    }

    #[test]
    fn calibration_scales_distance_and_threshold() {
        let now = Instant::now();
        // 預設參考 -59：-59 dBm ≈ 1 公尺
        let d = NearbyDevice::new("x".into(), None, -59, false, now);
        assert!((d.meters_calibrated(None) - 1.0).abs() < 0.01);
        // 校正成「1 公尺 = -49」：同樣 -59 是 10 dB 更弱 → 10^(10/25) ≈ 2.51 公尺
        assert!((d.meters_calibrated(Some(-49.0)) - 2.512).abs() < 0.01);
        assert_eq!(d.threshold_calibrated(-76, None), -76);
        assert_eq!(d.threshold_calibrated(-76, Some(-49.0)), -66);
    }

    #[test]
    fn calibration_reading_needs_full_fresh_window() {
        let mut c = Clock::new();
        let mut d = NearbyDevice::new("x".into(), None, -50, false, c.next());
        assert!(d.calibration_reading(c.0).is_none()); // 只有 1 筆
        for _ in 1..RSSI_WINDOW {
            d.update(-50, None, false, c.next());
        }
        assert_eq!(d.calibration_reading(c.0), Some((c.0, -50)));
        // 之後只收到無效的 127：last_seen 更新，但最新樣本已經舊了，不能拿來校正
        let later = c.0 + Duration::from_secs(5);
        d.update(127, None, false, later);
        assert_eq!(d.last_seen, later);
        assert!(d.calibration_reading(later).is_none());
    }

    #[test]
    fn calibration_reading_rejects_sparse_window() {
        // 每 4 秒才一筆：計數雖已超過 7，但 10 秒內只剩 3 筆樣本
        let t0 = Instant::now();
        let mut d = NearbyDevice::new("x".into(), None, -50, false, t0);
        let mut t = t0;
        for _ in 0..8 {
            t += Duration::from_secs(4);
            d.update(-50, None, false, t);
        }
        assert!(d.samples.len() < RSSI_WINDOW);
        assert!(d.calibration_reading(t).is_none());
    }

    #[test]
    fn burst_merge_updates_distance_too() {
        let mut clk = Clock::new();
        let mut d = dev(-59);
        for _ in 1..RSSI_WINDOW {
            d.update(-59, None, false, clk.next());
        }
        for _ in 0..3 {
            d.update(-70, None, false, clk.next()); // 4 筆 -59 + 3 筆 -70
        }
        let before = d.meters();
        let t = clk.next();
        d.update(-70, None, false, t); // 中位數翻成 -70
        assert_eq!(d.rssi, -70);
        // 同一次廣播較強的回報：中位數回到 -59，距離也要跟著回到合併前的值，而不是停在 -70 的結果
        d.update(-50, None, false, t + Duration::from_millis(1));
        assert_eq!(d.rssi, -59);
        let expected = smooth_distance(before, estimated_meters(-59));
        assert!((d.meters() - expected).abs() < 1e-9, "{} vs {expected}", d.meters());
    }
}
