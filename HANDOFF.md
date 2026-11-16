# AwayLock 交接文件

最後更新：2026-11-16

## 專案概述
macOS 狀態列 App。電腦解鎖期間，只要曾被掃描到的「允許裝置」（iPhone / Apple Watch / 藍牙配件）
其中任何一個超過設定距離或突然掃描不到，就自動鎖定螢幕。

- 語言與框架：Swift 5.9、SwiftUI + AppKit、CoreBluetooth、Swift Package（沒有 .xcodeproj）
- 最低系統：macOS 13
- 路徑：`~/Developer/AwayLock`

## 目前功能
- 狀態列圖示（不在 Dock）。左鍵選單只有 **Settings**、**Quit**。
- 啟動時顯示設定視窗，工具列樣式分頁：
  - **General**：開機時啟動、離開距離滑桿（0.5～5 公尺，每格 0.5）、Quit 按鈕
  - **Devices**：掃描附近 BLE 裝置、維護「允許列表」（加入／移除）
  - **Debug**：只存在開發版，顯示掃描與判定細節（含 dBm）
  - **About**
- 一般使用者介面只顯示公尺，不顯示 dBm。

## 鎖定規則（`ProximityMonitor.swift`）
1. 電腦每次解鎖（監聽 `com.apple.screenIsUnlocked`）開始一段新的監控期間。
2. 期間內，允許裝置被掃描到且在距離內，就「列入監控」。
3. 列入監控的裝置中，任一個連續 3 次檢查（每 5 秒一次）都超距或掃描不到（超過 15 秒沒收到廣播），就鎖定。
4. 從未在該期間被掃描到的裝置不會觸發鎖定。
5. 鎖定後清空監控名單；下次解鎖重新開始。
6. 鎖定方式：`pmset displaysleepnow`，需要「系統設定 → 鎖定畫面 → 需要密碼」設為「立即」。

距離由 RSSI 粗估：`d = 10^((-59 - rssi) / 25)`（1 公尺約 -59 dBm、環境係數 2.5）。
距離設定換算回門檻：`rssi = -59 - 25 * log10(公尺)`。誤差很大，僅供參考。

## 建置與執行
```bash
./build.sh dev       # 開發版 → build/AwayLock Dev.app（含 Debug，bundle id com.awaylock.app.dev）
./build.sh release   # 發布版 → build/AwayLock.app（不含任何 Debug 程式碼，bundle id com.awaylock.app）
```
- Debug 程式碼以 `#if DEBUG` 包住；發布版已用 `strings` 確認不含 Debug 字串。
- 兩者 ad-hoc 簽章。每次重新編譯後簽章會變，可能需要重新授權藍牙。
- App 圖示由 `scripts/make-icon.swift` 產生 `Resources/AppIcon.icns`。
- 「開機時啟動」使用 `SMAppService.mainApp`，App 需放在 `/Applications` 才可靠。

## 檔案結構
| 檔案 | 用途 |
|---|---|
| `Sources/AwayLock/main.swift` | 進入點（accessory 模式） |
| `AppDelegate.swift` | 狀態列圖示、選單、設定視窗 |
| `SettingsWindow.swift` | 工具列分頁控制器，Debug 分頁動態加入／移除 |
| `SettingsView.swift` | General / Devices / Debug / About 各分頁畫面 |
| `BLEScanner.swift` | CoreBluetooth 持續掃描，RSSI 以 0.7/0.3 平滑 |
| `AllowList.swift` | 允許列表（UserDefaults） |
| `AppSettings.swift` | 距離設定、Debug 開關、距離估算公式 |
| `ProximityMonitor.swift` | 鎖定判定邏輯 |

---

## 已知問題：Apple 裝置的識別碼會定期更換（尚未解決）

### 現象
iPhone / Apple Watch 加入允許列表後，過一段時間會變成「掃描不到」，
被監控中的裝置因此觸發鎖定，之後必須重新加入新的識別碼。

### 原因
- Apple 裝置的 BLE 廣播使用隨機私有位址（resolvable private address），為了防追蹤會定期更換。
  一般認為約每 **15 分鐘**一次，並可能在螢幕亮起、鎖定狀態改變時提早更換。
  **此週期未在使用者的裝置上實測。**
- CoreBluetooth 提供的 `peripheral.identifier` 由位址換算而來。位址一換，識別碼就變。
  對未配對的裝置，系統無法把新舊位址對應回同一台裝置。
- 允許列表存的是舊識別碼，所以舊的會永遠掃描不到。

### 影響
- 對 iPhone / Apple Watch，目前「允許列表」的做法不穩定，約 15 分鐘後就會誤判離開並鎖定。
- 使用固定位址的藍牙配件（耳機、喇叭、手環等，例如 Sony WH-1000XM6）不受影響。

### 已排除的做法
| 做法 | 結果 |
|---|---|
| 與 Mac 藍牙配對 iPhone 以讀 RSSI（`IOBluetoothDevice`） | iPhone 端顯示「配對失敗：不支援 Mac」，行不通。Apple Watch 本來也不能獨立與 Mac 配對 |
| 對已配對裝置主動 `openConnection()` | 會不斷在 iPhone / Mac 觸發配對請求視窗。已移除 |
| 用同一 Apple ID 辨識裝置 | 沒有公開 API 可列出同 Apple ID 的裝置，Find My / Continuity 不開放第三方 |
| CoreBluetooth 查系統配對清單 | 沒有此 API，識別碼為 App 專用的隨機值 |
| Nearby Interaction（UWB） | 需要兩端都有 UWB。據我所知 Mac 沒有 UWB 晶片，不適用（**未實際驗證**） |

### 可行方向（待決定）
1. **iPhone 配套 App（最準確）**
   iPhone 端做一個小 App，以固定的自訂 service UUID 持續廣播（可背景執行）。
   Mac 端只認這個 UUID，不受位址更換影響，且不會誤判別人的 Apple 裝置。
   代價：要另外開發 iOS App；免費 Apple ID 簽署每 7 天要重新安裝；Apple Watch 仍偵測不到。
2. **改用固定位址的配件**
   把不換位址的藍牙裝置放進允許列表。不需改程式，但要使用者隨身帶那個配件。
3. **維持現狀並降低誤鎖**
   在識別碼更換時自動重新綁定（例如以廣播內容或訊號特徵比對）。可靠度低，**尚未驗證可行**。
4. **接受限制**
   僅文件化提醒使用者，Apple 裝置需定期重新加入。

### 建議的下一步
1. 用開發版 Debug 分頁實測：記下 iPhone 識別碼前幾碼，隔 10～20 分鐘再看是否換了，確認實際週期。
2. 依實測結果決定走方向 1 或 2。

## 其他待辦／未驗證
- 設定視窗的實際外觀、藍牙掃描結果、實際鎖定行為，都只確認過「編譯成功並能啟動」，沒有實機驗證。
- 「開機時啟動」尚未在 `/Applications` 下測試。
- 鎖定使用 `pmset displaysleepnow`，是否在所有 macOS 版本都會要求密碼待確認。
- 發布前需正式簽章與公證（目前僅 ad-hoc 簽章）。
- 專案尚未建立 git repository。
