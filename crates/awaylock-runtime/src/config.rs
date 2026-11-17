//! 設定與允許列表的存取（JSON 檔，放在使用者的設定資料夾）。
//! 開發版與發布版使用不同的資料夾，互不干擾。

use awaylock_core::{AllowList, Settings};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

#[cfg(debug_assertions)]
const APP_DIR: &str = "awaylock Dev";
#[cfg(not(debug_assertions))]
const APP_DIR: &str = "awaylock";

fn dir() -> Option<PathBuf> {
    // 測試或特殊情況可用環境變數指定資料夾，避免動到使用者的真實設定
    if let Some(custom) = std::env::var_os("AWAYLOCK_CONFIG_DIR") {
        return Some(PathBuf::from(custom));
    }
    ProjectDirs::from("app", "awaylock", APP_DIR).map(|d| d.config_dir().to_path_buf())
}

fn read<T: serde::de::DeserializeOwned + Default>(file: &str) -> T {
    dir()
        .and_then(|d| fs::read_to_string(d.join(file)).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write<T: serde::Serialize>(file: &str, value: &T) {
    let Some(d) = dir() else { return };
    if fs::create_dir_all(&d).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string_pretty(value) {
        let _ = fs::write(d.join(file), json);
    }
}

pub fn load_settings() -> Settings {
    read::<Settings>("settings.json").sanitized()
}

pub fn save_settings(s: &Settings) {
    write("settings.json", s);
}

pub fn load_allowlist() -> AllowList {
    read("allowlist.json")
}

pub fn save_allowlist(l: &AllowList) {
    write("allowlist.json", l);
}
