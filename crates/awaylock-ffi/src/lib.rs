//! C 介面。給 Swift 原生介面呼叫；狀態以 JSON 字串傳遞。
//!
//! 所有回傳的 `char*` 都要用 `awaylock_free_string` 釋放。
//! 對應的標頭檔：`apps/macos/Sources/cawaylock/include/awaylock.h`。

use awaylock_runtime::Runtime;
use std::ffi::{c_char, CStr, CString};
use std::sync::OnceLock;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn to_c(s: String) -> *mut c_char {
    CString::new(s).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `p` 必須是有效、以 NUL 結尾的 C 字串，或 NULL。
unsafe fn from_c(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    Some(CStr::from_ptr(p).to_string_lossy().into_owned())
}

/// 啟動執行層（藍牙掃描與背景檢查）。可重複呼叫，只會啟動一次。
#[no_mangle]
pub extern "C" fn awaylock_start() {
    RUNTIME.get_or_init(Runtime::start);
}

/// 目前是否為 debug 編譯的函式庫（發布版為 false）。
#[no_mangle]
pub extern "C" fn awaylock_is_debug_build() -> bool {
    cfg!(debug_assertions)
}

/// 一般狀態 JSON：距離設定、藍牙狀態、允許的裝置、附近的裝置。
#[no_mangle]
pub extern "C" fn awaylock_state_json() -> *mut c_char {
    to_c(RUNTIME.get().map(Runtime::state_json).unwrap_or_else(|| "{}".into()))
}

/// 除錯狀態 JSON。發布版固定回傳 `{}`。
#[no_mangle]
pub extern "C" fn awaylock_debug_json() -> *mut c_char {
    to_c(RUNTIME.get().map(Runtime::debug_json).unwrap_or_else(|| "{}".into()))
}

/// 設定離開距離（公尺）。會自動限制在 1～5 並對齊 0.5。
#[no_mangle]
pub extern "C" fn awaylock_set_distance(meters: f64) {
    if let Some(rt) = RUNTIME.get() {
        rt.set_distance(meters);
    }
}

/// 「附近的裝置」是否只顯示手機與手錶。
#[no_mangle]
pub extern "C" fn awaylock_set_only_phones_watches(on: bool) {
    if let Some(rt) = RUNTIME.get() {
        rt.set_only_phones_watches(on);
    }
}

/// 把裝置加入允許列表。
///
/// # Safety
/// `id`、`name` 必須是有效、以 NUL 結尾的 C 字串。
#[no_mangle]
pub unsafe extern "C" fn awaylock_add_device(id: *const c_char, name: *const c_char) {
    if let (Some(rt), Some(id), Some(name)) = (RUNTIME.get(), from_c(id), from_c(name)) {
        rt.add_device(&id, &name);
    }
}

/// 從允許列表移除裝置。
///
/// # Safety
/// `id` 必須是有效、以 NUL 結尾的 C 字串。
#[no_mangle]
pub unsafe extern "C" fn awaylock_remove_device(id: *const c_char) {
    if let (Some(rt), Some(id)) = (RUNTIME.get(), from_c(id)) {
        rt.remove_device(&id);
    }
}

/// 釋放本函式庫回傳的字串。
///
/// # Safety
/// `s` 必須是本函式庫回傳的指標，且只能釋放一次。
#[no_mangle]
pub unsafe extern "C" fn awaylock_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}
