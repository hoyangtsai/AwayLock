//! 掃描到的藍牙裝置。

use std::time::Instant;

/// Apple 的藍牙廠商代碼（Company Identifier）。
pub const APPLE_COMPANY_ID: u16 = 0x004C;

#[derive(Debug, Clone, PartialEq)]
pub struct NearbyDevice {
    /// 平台提供的識別碼（macOS 為 UUID，Linux 為藍牙位址）。
    /// Apple 裝置的識別碼會定期更換。
    pub id: String,
    pub name: Option<String>,
    /// 平滑後的訊號強度（dBm）。
    pub rssi: i32,
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
        Self { id, name, rssi, last_seen: now, is_apple, apple_types: Vec::new() }
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

    /// 收到新的廣播：更新訊號（指數平滑）、名稱與時間。
    pub fn update(&mut self, rssi: i32, name: Option<String>, is_apple: bool, now: Instant) {
        self.rssi = smooth(self.rssi, rssi);
        self.last_seen = now;
        if let Some(n) = name.filter(|n| !n.is_empty()) {
            self.name = Some(n);
        }
        self.is_apple = self.is_apple || is_apple;
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

/// 指數平滑：舊值 70%、新值 30%。
pub fn smooth(old: i32, new: i32) -> i32 {
    (f64::from(old) * 0.7 + f64::from(new) * 0.3).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_moves_toward_new_value() {
        assert_eq!(smooth(-60, -90), -69);
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
}
