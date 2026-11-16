//! 允許列表：只有列表中的裝置才會影響自動鎖定。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllowedDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AllowList {
    devices: Vec<AllowedDevice>,
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

    /// 加入裝置；已存在則回傳 false。
    pub fn add(&mut self, id: impl Into<String>, name: impl Into<String>) -> bool {
        let id = id.into();
        if self.contains(&id) {
            return false;
        }
        self.devices.push(AllowedDevice { id, name: name.into() });
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
}
