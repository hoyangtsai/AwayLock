# AwayLock

人離開電腦時，自動鎖定 Mac 的狀態列 App。

用藍牙低功耗（BLE）掃描你隨身裝置的訊號。電腦解鎖期間，只要你加入「允許列表」的裝置中，
有任何一個超過設定距離或突然掃描不到，就自動鎖定螢幕。

> **狀態：開發中。** 已能編譯並啟動，但藍牙掃描與實際鎖定行為尚未完整實機驗證。
> 已知限制請見下方「限制」與 [HANDOFF.md](HANDOFF.md)。

## 功能
- 只在狀態列顯示，不佔用 Dock。左鍵點圖示，選單只有 **Settings** 與 **Quit**。
- 設定視窗：
  - **General**：開機時啟動、離開距離（0.5～5 公尺，每格 0.5 公尺）
  - **Devices**：掃描附近的藍牙裝置，維護「允許列表」
  - **About**
- 距離以公尺顯示，不需要看懂訊號強度數字。

## 鎖定規則
1. 電腦每次解鎖後，開始一段新的監控期間。
2. 期間內，允許列表中的裝置只要被掃描到、且在設定距離內，就列入監控。
3. 列入監控的裝置中，任何一個超過設定距離，或突然掃描不到（超過 15 秒沒收到廣播），
   並連續 3 次檢查（每 5 秒一次）都如此，就鎖定螢幕。
4. 期間內從沒被掃描到的裝置不會觸發鎖定。
5. 鎖定後清空監控名單，下次解鎖重新開始。

## 需求
- macOS 13 以上
- Xcode（含 Swift 5.9 以上工具鏈）
- 第一次啟動時允許藍牙權限
- 「系統設定 → 鎖定畫面 → 需要密碼」設為「**立即**」。App 是讓顯示器休眠（`pmset displaysleepnow`），
  沒有這項設定就不會要求密碼。

## 建置與執行
```bash
./build.sh release   # 發布版 → build/AwayLock.app
./build.sh dev       # 開發版 → build/AwayLock Dev.app（含 Debug 分頁）
```

```bash
open build/AwayLock.app
```

- **發布版**不含任何 Debug 程式碼。
- **開發版**可在 General 勾選「Debug 模式」，多出 Debug 分頁，顯示掃描到的所有裝置、
  允許裝置的訊號與估算距離、判定狀態等細節。開發版使用不同的 bundle id，不會和發布版互相干擾。
- 兩者都是 ad-hoc 簽章。每次重新編譯後可能需要重新授權藍牙。
- 「開機時啟動」需要把 App 放進 `/Applications` 才可靠。

## 使用方式
1. 開啟 App，到 **Devices** 分頁。
2. 把手機、手錶或藍牙配件靠近 Mac，在「附近的裝置」找距離最近的那個，按「加入」。
3. 在 **General** 調整離開距離。
4. 解鎖狀態下拿著裝置走遠，超過距離後螢幕就會鎖定。

## 限制
- **Apple 裝置的識別碼會定期更換。** iPhone / Apple Watch 為了防追蹤，BLE 位址約每 15 分鐘更換一次
  （實際週期未實測），加入允許列表的識別碼會失效，需要重新加入，否則會被判定為「掃描不到」而觸發鎖定。
  固定位址的藍牙配件（耳機、喇叭、手環等）不受影響。詳見 [HANDOFF.md](HANDOFF.md)。
- iPhone 和 Apple Watch 無法與 Mac 藍牙配對，因此讀不到配對式的連線訊號。
- 距離是由訊號強度粗估（1 公尺約 -59 dBm、環境係數 2.5），牆壁、人體遮擋、手機放口袋都會造成誤差，僅供參考。

## 專案結構
```
Package.swift
build.sh                       打包成 .app（release / dev）
Resources/                     Info.plist、AppIcon.icns
scripts/make-icon.swift        產生 App 圖示
Sources/AwayLock/
  main.swift                   進入點
  AppDelegate.swift            狀態列圖示、選單、設定視窗
  SettingsWindow.swift         工具列樣式分頁
  SettingsView.swift           各分頁畫面
  BLEScanner.swift             CoreBluetooth 掃描
  AllowList.swift              允許列表
  AppSettings.swift            設定與距離估算
  ProximityMonitor.swift       鎖定判定
```

另有 `proximity-lock.swift`：最早的命令列原型（讀取已配對裝置的 RSSI），已被 App 取代，僅保留供參考。

## 授權
尚未指定。
