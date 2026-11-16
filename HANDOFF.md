# AwayLock 交接文件

最後更新：2026-11-16

## 專案概述
狀態列 App。電腦解鎖期間，只要曾被掃描到的「允許裝置」（iPhone / Apple Watch / 藍牙配件）
其中任何一個超過設定距離或突然掃描不到，就自動鎖定螢幕。

- **核心邏輯已用 Rust 重寫並分層**（原本全是 Swift），為未來支援 Linux 做準備。
- **macOS 介面維持 Swift 原生（SwiftUI / AppKit）**，因為 egui 做不出原生外觀（使用者要求視窗和原本的 Swift 版一樣）。
  Swift 透過 C 介面呼叫 Rust。
- 路徑：`~/Developer/AwayLock`

## 架構（依賴方向：介面 → runtime → platform → core）
| 位置 | 職責 |
|---|---|
| `crates/awaylock-core` | 純邏輯，無作業系統依賴：距離換算、裝置、允許列表、設定、鎖定判定狀態機（`monitor.rs`）、平台要實作的 trait（`ports.rs`）。**23 個單元測試。** |
| `crates/awaylock-platform` | 實作 core 的 trait。藍牙用 btleplug（macOS / Linux 共用）；鎖定、鎖定狀態、開機啟動依平台用 `cfg(target_os)` 切換。 |
| `crates/awaylock-runtime` | 組合 core 與 platform：背景檢查執行緒、設定與允許列表存檔、給介面用的狀態檢視（`view.rs`，JSON）。**3 個測試**（含「一般狀態不含 dBm」）。 |
| `crates/awaylock-ffi` | C 介面（cdylib）。函式見 `apps/macos/Sources/CAwayLock/include/awaylock.h`，狀態以 JSON 字串傳遞。 |
| `apps/macos` | Swift 原生介面（狀態列、設定視窗）。`Core.swift` 每秒輪詢 Rust 取得狀態。 |
| `crates/awaylock-app` | egui 備用介面，**預留給 Linux**；macOS 不使用，也沒有維護到與 Swift 版同等。 |

新增平台時，只需在 `awaylock-platform` 補實作；Linux 介面可用 `awaylock-app`（egui）或另外做 GTK/Qt。

## 目前功能
- 狀態列圖示（不在 Dock，`LSUIElement`）。左鍵選單只有 **Settings**、**Quit**。
- 啟動時顯示設定視窗，工具列樣式分頁：
  - **General**：開機時啟動（Swift 的 `SMAppService`）、離開距離滑桿（0.5～5 公尺，每格 0.5）、Quit
  - **Devices**：掃描附近 BLE 裝置、維護允許列表。**一般模式一律只顯示手機與手錶，沒有開關**；
    只有開發版開啟 Debug 模式後，才能選「只顯示手機與手錶」或「全部裝置」（Rust 端在發布版強制過濾，
    `set_only_phones_watches` 不做事）。
    分類邏輯在 `crates/awaylock-core/src/device.rs`（`classify`）：**只看名稱**含 iphone / phone / watch 才算手機或手錶
    （先排除 headphone、airpods、macbook 等其他關鍵字）。有名稱但不像手機的（例如 HomePod 的「客廳」）一律不算。
    沒有名稱的 Apple 裝置不算手機：Nearby Info、Handoff 等廣播代碼所有 Apple 裝置都會送，無法區分（曾因此把 HomePod、
    同一支手機的其他廣播位址誤判成手機）。廣播代碼只用來辨認耳機、AirTag、AirPlay 裝置（0x07 / 0x12 / 0x09，社群逆向研究，
    非官方、未驗證）。開發版 Debug 分頁會顯示每個裝置的推測種類與原始代碼。
    **副作用**：iPhone 識別碼更換後，新的那一筆若沒有名稱就不會出現在一般模式的列表（需要靠開發版找出來）。
  - **Debug**：只存在開發版，含 dBm 與判定細節
  - **About**
- 一般使用者介面只顯示公尺，不顯示 dBm。
- 距離與允許列表由 Rust 存成 JSON（`directories` crate 決定位置；開發版與發布版用不同資料夾）。
  測試用環境變數 `AWAYLOCK_CONFIG_DIR` 指向暫存資料夾，避免動到使用者設定。

## 鎖定規則（`crates/awaylock-core/src/monitor.rs`）
1. 電腦每次解鎖開始一段新的監控期間（以輪詢鎖定狀態偵測，每 5 秒一次）。
2. 期間內，允許裝置被掃描到且在距離內，就「列入監控」。
3. 列入監控的裝置中，任一個連續 3 次檢查都超距或掃描不到（超過 15 秒沒收到廣播），就鎖定。
4. 從未在該期間被掃描到的裝置不會觸發鎖定。
5. 發出鎖定指令後進入「等待鎖定生效」，電腦真的鎖定後才進入「已鎖定」，再解鎖才重新開始；
   20 秒內電腦沒鎖定（例如沒設「需要密碼：立即」）就放棄等待並重新開始。

距離由 RSSI 粗估：`d = 10^((-59 - rssi) / 25)`；距離設定換算門檻：`rssi = -59 - 25 * log10(公尺)`。誤差很大，僅供參考。

## 建置與執行
```bash
scripts/bundle-macos.sh dev       # 開發版 → dist/AwayLock Dev.app（含 Debug，bundle id com.awaylock.app.dev）
scripts/bundle-macos.sh release   # 發布版 → dist/AwayLock.app（不含 Debug 程式碼）
cargo test --workspace            # 核心邏輯與狀態檢視測試
```
- 腳本流程：`cargo build -p awaylock-ffi` → Swift 連結 `libawaylock_ffi.dylib` → 組成 `.app`
  （執行檔在 `Contents/MacOS`，動態庫在 `Contents/Frameworks`，rpath 為 `@executable_path/../Frameworks`）。
- **必須用 .app 執行**：沒有 `Info.plist` 的藍牙權限說明會被系統直接中止（exit 134）。
  這也表示在 Claude 的沙盒終端機直接執行 Rust 二進位檔會中止，不是程式 bug。
- **根目錄 `Cargo.toml` 的 `[profile.release] strip = "none"` 不能刪**：cargo 預設會對 release 動態庫做 strip，
  Xcode 的連結器會拒絕（`mis-aligned LINKEDIT string pool`）。
- `awaylock-ffi/build.rs` 在連結時設定 `-install_name @rpath/libawaylock_ffi.dylib`，不要改用 `install_name_tool`
  事後修改（同樣會破壞檔案）。
- 發布版已用 `strings` 確認 Swift 執行檔與 Rust 動態庫都不含 Debug 字串。
- 兩者 ad-hoc 簽章；每次重新編譯後可能需要重新授權藍牙。
- 「開機時啟動」需要 App 放在 `/Applications` 才可靠。
- App 圖示：`scripts/make-icon.swift` 產生 `assets/AppIcon.icns`。

## 驗證狀態
- **已驗證**：Rust 核心與狀態檢視共 26 個測試通過；開發版與發布版都能編譯、打包並啟動；
  Swift 設定視窗的 General 分頁畫面與原本 Swift 版相同（原生工具列分頁、Debug 分頁在開發版出現）；
  發布版不含 Debug 字串。
- **未驗證**：
  - 藍牙掃描實際結果（btleplug 在 macOS 是否持續回報 RSSI 更新）
  - 過濾在使用者實機上：已知「Hoyang's iPhone 17 Pro」「Hoyang's Apple Watch」會因名稱被顯示；其他情況（名稱消失、更換識別碼後）未驗證
  - Swift 與 Rust 之間的資料流在畫面上的呈現（Devices / Debug 分頁的實際內容）
  - 實際鎖定行為、鎖定／解鎖偵測（`CGSSessionScreenIsLocked`）
  - 「開機時啟動」
  - **Linux 完全未編譯、未測試**（`loginctl`、BlueZ、狀態列圖示在 GNOME 需擴充套件）；`awaylock-app` 在 macOS 能編譯，
    但沒有經過完整驗證。

---

## 已知問題：Apple 裝置的識別碼會定期更換（尚未解決）

### 現象
iPhone / Apple Watch 加入允許列表後，過一段時間會變成「掃描不到」，
被監控中的裝置因此觸發鎖定，之後必須重新加入新的識別碼。

### 原因
- Apple 裝置的 BLE 廣播使用隨機私有位址（resolvable private address），為了防追蹤會定期更換。
  一般認為約每 **15 分鐘**一次，並可能在螢幕亮起、鎖定狀態改變時提早更換。
  **此週期未在使用者的裝置上實測。**
- btleplug / CoreBluetooth 提供的識別碼由位址換算而來。位址一換，識別碼就變。
  對未配對的裝置，系統無法把新舊位址對應回同一台裝置。
- 允許列表存的是舊識別碼，所以舊的會永遠掃描不到。

### 影響
- 對 iPhone / Apple Watch，目前「允許列表」的做法不穩定，約 15 分鐘後就會誤判離開並鎖定。
- 使用固定位址的藍牙配件（耳機、喇叭、手環等，例如 Sony WH-1000XM6）不受影響。

### 已排除的做法
| 做法 | 結果 |
|---|---|
| 與 Mac 藍牙配對 iPhone 以讀 RSSI | iPhone 端顯示「配對失敗：不支援 Mac」，行不通。Apple Watch 本來也不能獨立與 Mac 配對 |
| 對已配對裝置主動連線 | 會不斷在 iPhone / Mac 觸發配對請求視窗（舊 Swift 版踩過） |
| 用同一 Apple ID 辨識裝置 | 沒有公開 API 可列出同 Apple ID 的裝置，Find My / Continuity 不開放第三方 |
| 查系統配對清單 | CoreBluetooth 沒有此 API，識別碼為 App 專用的隨機值 |
| Nearby Interaction（UWB） | 需要兩端都有 UWB。據我所知 Mac 沒有 UWB 晶片，不適用（**未實際驗證**） |

### 可行方向（待決定）
1. **iPhone 配套 App（最準確）**：iPhone 端以固定的自訂 service UUID 持續廣播，Mac 端只認這個 UUID。
   代價：要另外開發 iOS App；免費 Apple ID 簽署每 7 天要重新安裝；Apple Watch 仍偵測不到。
2. **改用固定位址的配件**：把不換位址的藍牙裝置放進允許列表。不需改程式。
3. **自動重新綁定**：識別碼更換時以廣播內容或訊號特徵比對。可靠度低，**尚未驗證可行**。
4. **接受限制**：僅文件化提醒使用者，Apple 裝置需定期重新加入。

### 建議的下一步
1. 用開發版 Debug 分頁實測：記下 iPhone 識別碼前幾碼，隔 10～20 分鐘再看是否換了，確認實際週期。
2. 依實測結果決定走方向 1 或 2。

## 其他待辦
- 發布前需正式簽章與公證（目前僅 ad-hoc 簽章）。
- 補上授權（LICENSE）。
- Linux：補編譯驗證，決定介面方案（egui 或 GTK/Qt），評估狀態列圖示。
- 專案尚未推送到 GitHub（`origin` 已設為 `git@github.com:hoyangtsai/AwayLock.git`），變更也尚未 commit。
