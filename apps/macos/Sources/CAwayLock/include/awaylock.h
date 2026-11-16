// AwayLock 核心（Rust）的 C 介面。對應 crates/awaylock-ffi/src/lib.rs。
// 回傳的 char* 都要用 awaylock_free_string 釋放。
#ifndef AWAYLOCK_H
#define AWAYLOCK_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// 啟動藍牙掃描與背景檢查。可重複呼叫，只會啟動一次。
void awaylock_start(void);

// 函式庫是否為 debug 編譯（發布版為 false）。
bool awaylock_is_debug_build(void);

// 一般狀態 JSON：距離設定、藍牙狀態、允許的裝置、附近的裝置。
char *awaylock_state_json(void);

// 除錯狀態 JSON。發布版固定回傳 "{}"。
char *awaylock_debug_json(void);

// 設定離開距離（公尺），會自動限制在 0.5～5 並對齊 0.5。
void awaylock_set_distance(double meters);

// 「附近的裝置」是否只顯示手機與手錶（依廣播內容推測）。
void awaylock_set_only_phones_watches(bool on);

// 加入／移除允許列表中的裝置。
void awaylock_add_device(const char *id, const char *name);
void awaylock_remove_device(const char *id);

// 釋放函式庫回傳的字串。
void awaylock_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif
