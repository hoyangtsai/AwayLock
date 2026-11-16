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
}

impl NearbyDevice {
    pub fn new(id: String, name: Option<String>, rssi: i32, is_apple: bool, now: Instant) -> Self {
        Self { id, name, rssi, last_seen: now, is_apple }
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
    fn display_name_fallbacks() {
        let now = Instant::now();
        let d = NearbyDevice::new("12345678-aaaa".into(), None, -60, true, now);
        assert_eq!(d.display_name(), "Apple 裝置 12345678");
        let d = NearbyDevice::new("12345678-aaaa".into(), None, -60, false, now);
        assert_eq!(d.display_name(), "未命名裝置 12345678");
    }
}
