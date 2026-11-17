//! 允許列表：只有列表中的裝置才會影響自動鎖定。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllowedDevice {
    pub id: String,
    pub name: String,
    /// 使用者在 1 公尺處校正得到的 RSSI（dBm）。沒有校正時用裝置種類的預設值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rssi_at_1m: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AllowList {
    devices: Vec<AllowedDevice>,
    /// 依裝置名稱記住的 1 公尺校正值。Apple 裝置的識別碼會定期更換，
    /// 用同一個名稱重新加入時沿用之前的校正，不必再校正一次。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    calibrations_by_name: BTreeMap<String, f64>,
}

impl AllowList {
    pub fn devices(&self) -> &[AllowedDevice] {
        &self.devices
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.devices.iter().any(|d| d.id == id)
    }

    /// 加入裝置；已存在則回傳 false。之前以相同名稱校正過的話，沿用該校正值。
    pub fn add(&mut self, id: impl Into<String>, name: impl Into<String>) -> bool {
        let id = id.into();
        if self.contains(&id) {
            return false;
        }
        let name = name.into();
        let rssi_at_1m = self.calibrations_by_name.get(&name).copied();
        self.devices.push(AllowedDevice { id, name, rssi_at_1m });
        true
    }

    pub fn get(&self, id: &str) -> Option<&AllowedDevice> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// 設定或清除（`None`）裝置的 1 公尺校正值，並記在裝置名稱下；裝置不存在則回傳 false。
    pub fn set_calibration(&mut self, id: &str, rssi_at_1m: Option<f64>) -> bool {
        let Some(d) = self.devices.iter_mut().find(|d| d.id == id) else {
            return false;
        };
        d.rssi_at_1m = rssi_at_1m;
        match rssi_at_1m {
            Some(r) => self.calibrations_by_name.insert(d.name.clone(), r),
            None => self.calibrations_by_name.remove(&d.name),
        };
        true
    }

    /// 移除裝置；不存在則回傳 false。
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        self.devices.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_remove_contains() {
        let mut l = AllowList::default();
        assert!(l.add("a", "Phone"));
        assert!(!l.add("a", "Phone again"));
        assert!(l.contains("a"));
        assert!(l.remove("a"));
        assert!(!l.remove("a"));
        assert!(l.is_empty());
    }

    #[test]
    fn serde_round_trip_is_stable() {
        let mut l = AllowList::default();
        l.add("a", "Phone");
        let json = serde_json::to_string(&l).unwrap();
        let back: AllowList = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn calibration_is_saved_and_cleared() {
        let mut l = AllowList::default();
        l.add("a", "Phone");
        assert!(l.set_calibration("a", Some(-48.0)));
        assert!(!l.set_calibration("missing", Some(-48.0)));
        let back: AllowList = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back.get("a").unwrap().rssi_at_1m, Some(-48.0));
        assert!(l.set_calibration("a", None));
        assert_eq!(l.get("a").unwrap().rssi_at_1m, None);
    }

    #[test]
    fn old_files_without_calibration_still_load() {
        let l: AllowList = serde_json::from_str(r#"{"devices":[{"id":"a","name":"Phone"}]}"#).unwrap();
        assert_eq!(l.get("a").unwrap().rssi_at_1m, None);
    }

    #[test]
    fn re_added_device_keeps_calibration_by_name() {
        let mut l = AllowList::default();
        l.add("old-id", "Hoyang’s iPhone");
        l.set_calibration("old-id", Some(-47.0));
        // 識別碼更換：移除舊的、加入新的（同名）
        l.remove("old-id");
        l.add("new-id", "Hoyang’s iPhone");
        assert_eq!(l.get("new-id").unwrap().rssi_at_1m, Some(-47.0));
        // 舊的還在列表中時加入新的，也一樣沿用
        l.add("third-id", "Hoyang’s iPhone");
        assert_eq!(l.get("third-id").unwrap().rssi_at_1m, Some(-47.0));
        // 其他名稱不受影響
        l.add("watch", "Hoyang’s Apple Watch");
        assert_eq!(l.get("watch").unwrap().rssi_at_1m, None);
        // 記憶會存檔
        let back: AllowList = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
    }

    #[test]
    fn reset_forgets_calibration_for_the_name() {
        let mut l = AllowList::default();
        l.add("a", "Phone");
        l.set_calibration("a", Some(-47.0));
        l.set_calibration("a", None);
        l.remove("a");
        l.add("b", "Phone");
        assert_eq!(l.get("b").unwrap().rssi_at_1m, None);
    }
}
