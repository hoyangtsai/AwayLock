# AwayLock

人離開電腦時，自動鎖定電腦的狀態列 App。目前支援 macOS，架構已為 Linux 預留。

用藍牙低功耗（BLE）掃描你隨身裝置的訊號。電腦解鎖期間，只要你加入「允許列表」的裝置中，
有任何一個超過設定距離或突然掃描不到，就自動鎖定螢幕。

> **狀態：開發中。** 核心邏輯用 Rust 撰寫，macOS 介面是 Swift 原生視窗。已能編譯並啟動，
> 但藍牙掃描與實際鎖定行為尚未完整實機驗證。已知限制請見下方「限制」與 [HANDOFF.md](HANDOFF.md)。

## 功能
- 只在狀態列顯示，不佔用 Dock。左鍵點圖示，選單只有 **Settings** 與 **Quit**。
- 設定視窗（原生 macOS 外觀，工具列樣式分頁；關閉視窗後 App 仍留在狀態列）：
  - **General**：開機時啟動、離開距離（0.5～5 公尺，每格 0.5 公尺）
  - **Devices**：掃描附近的藍牙裝置，維護「允許列表」。只列出名稱顯示為手機或手錶的裝置
  - **About**
- 距離以公尺顯示，不需要看懂訊號強度數字。
- 開發版另有 **Debug** 分頁（顯示掃描到的裝置、即時訊號、判定細節）。發布版完全不含這些程式碼。

## 鎖定規則
1. 電腦每次解鎖後，開始一段新的監控期間。
2. 期間內，允許列表中的裝置只要被掃描到、且在設定距離內，就列入監控。
3. 列入監控的裝置中，任何一個超過設定距離，或突然掃描不到（超過 15 秒沒收到廣播），
   並連續 3 次檢查（每 5 秒一次）都如此，就鎖定螢幕。
4. 期間內從沒被掃描到的裝置不會觸發鎖定。
5. 鎖定後清空監控名單；電腦真的鎖定、再解鎖後才重新開始。

## 需求
- Rust（stable）與 Xcode（Swift 5.9 以上）
- macOS 13 以上；第一次啟動時允許藍牙權限
- 「系統設定 → 鎖定畫面 → 需要密碼」設為「**立即**」。App 是讓顯示器休眠（`pmset displaysleepnow`），
  沒有這項設定就不會要求密碼。

## 建置與執行
```bash
scripts/bundle-macos.sh release   # 發布版 → dist/AwayLock.app
scripts/bundle-macos.sh dev       # 開發版 → dist/AwayLock Dev.app（含 Debug 分頁）
```

```bash
open dist/AwayLock.app
```

- 腳本會先編譯 Rust 核心成動態函式庫，再編譯 Swift 介面並連結它，最後組成 `.app`。
- 必須用打包出來的 `.app` 執行：macOS 需要 `Info.plist` 裡的藍牙權限說明，沒有的話程式會直接中止。
- 開發版使用不同的 bundle id 與設定資料夾，不會和發布版互相干擾。
- Debug 功能在 Rust 端以 `#[cfg(debug_assertions)]`、在 Swift 端以 `#if DEBUG` 控制，
  只有開發版會編譯進去。
- 兩者都是 ad-hoc 簽章。每次重新編譯後可能需要重新授權藍牙。
- 「開機時啟動」需要把 App 放進 `/Applications` 才可靠。

測試核心邏輯：

```bash
cargo test --workspace
```

## 使用方式
1. 開啟 App，到 **Devices** 分頁。
2. 把手機、手錶或藍牙配件靠近 Mac，在「附近的裝置」找距離最近的那個，按「加入」。
3. 在 **General** 調整離開距離。
4. 解鎖狀態下拿著裝置走遠，超過距離後螢幕就會鎖定。

## 架構

核心邏輯用 Rust 寫成 Cargo workspace，分層；各平台的原生介面放在 `apps/`。
依賴方向：介面 → `runtime` → `platform` → `core`。

```
crates/
  awaylock-core/       核心層：純邏輯，不依賴任何作業系統 API，有單元測試
    distance.rs          RSSI ↔ 距離換算
    device.rs            掃描到的裝置、訊號平滑
    allowlist.rs         允許列表
    settings.rs          設定（距離 0.5～5 公尺）
    monitor.rs           鎖定判定狀態機
    ports.rs             「埠」：平台需要實作的 trait
  awaylock-platform/   平台層：實作 core 的 trait，依平台切換
    ble.rs               btleplug 藍牙掃描（macOS / Linux 共用）
    locker.rs            鎖定螢幕
    lockstate.rs         查詢電腦是否已鎖定
    autostart.rs         開機啟動
  awaylock-runtime/    執行層：組合核心與平台層（背景檢查、設定存取、給介面用的狀態檢視）
  awaylock-ffi/        C 介面（動態函式庫），狀態以 JSON 傳遞，給 Swift 呼叫
  awaylock-app/        跨平台備用介面（egui），預留給 Linux；macOS 不使用
apps/
  macos/               macOS 原生介面（SwiftUI / AppKit），透過 C 介面呼叫 Rust
assets/AppIcon.icns    App 圖示（由 scripts/make-icon.swift 產生）
packaging/macos/       macOS 打包用的 Info.plist
scripts/               打包腳本、圖示產生腳本、早期原型
```

| 能力 | macOS | Linux（尚未驗證） |
|---|---|---|
| 藍牙掃描 | btleplug（CoreBluetooth） | btleplug（BlueZ） |
| 鎖定螢幕 | `pmset displaysleepnow` | `loginctl lock-session` |
| 鎖定狀態 | `CGSessionCopyCurrentDictionary` | `loginctl show-session -p LockedHint` |
| 開機啟動 | Swift 端 `SMAppService` | XDG autostart |
| 介面 | Swift 原生（`apps/macos`） | egui（`crates/awaylock-app`） |

要支援新平台，只需要在 `awaylock-platform` 補上對應的實作；核心判定邏輯不用改。

## 限制
- **Apple 裝置的識別碼會定期更換。** iPhone / Apple Watch 為了防追蹤，BLE 位址約每 15 分鐘更換一次
  （實際週期未實測），加入允許列表的識別碼會失效，需要重新加入，否則會被判定為「掃描不到」而觸發鎖定。
  固定位址的藍牙配件（耳機、喇叭、手環等）不受影響。詳見 [HANDOFF.md](HANDOFF.md)。
- iPhone 和 Apple Watch 無法與 Mac 藍牙配對，因此讀不到配對式的連線訊號。
- 「只顯示手機與手錶」是依**裝置名稱**判斷的（含 iPhone、phone、watch，且不像耳機、Mac 等）。
  沒有名稱的裝置（例如更換識別碼後的 iPhone）不會顯示，因為廣播內容分不出它是手機、iPad 還是 Mac。
  一般使用者無法關閉這個過濾；開發版開啟 Debug 模式後，可以在 Devices 分頁選擇顯示全部裝置。
- 距離是由訊號強度粗估（1 公尺約 -59 dBm、環境係數 2.5），牆壁、人體遮擋、手機放口袋都會造成誤差，僅供參考。
- Linux 版尚未編譯或測試。Linux 的狀態列圖示在 GNOME 上需要額外的擴充套件。

## 授權
尚未指定。
