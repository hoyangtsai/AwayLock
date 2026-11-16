// proximity-lock.swift
// 藍牙距離偵測：配對裝置離開範圍（斷線或訊號太弱）時自動鎖定 Mac。
//
// 編譯：  swiftc -O proximity-lock.swift -o proximity-lock
// 列出：  ./proximity-lock --list
// 執行：  ./proximity-lock --device "我的 iPhone" --threshold -80 --misses 3 --interval 5
//
// 前置條件：「系統設定 → 鎖定畫面 → 需要密碼」設為「立即」，
// 否則螢幕休眠後不會要求密碼。

import Foundation
@preconcurrency import IOBluetooth

// MARK: - 參數

struct Config {
    var device: String? = nil
    var threshold: Int = -80      // RSSI 低於此值視為「離開」
    var misses: Int = 3           // 連續幾次判定離開才鎖定
    var interval: TimeInterval = 5
    var list = false
    var verbose = false
}

func parseArgs() -> Config {
    var c = Config()
    var it = CommandLine.arguments.dropFirst().makeIterator()
    while let a = it.next() {
        switch a {
        case "--list": c.list = true
        case "--device": c.device = it.next()
        case "--threshold": c.threshold = Int(it.next() ?? "") ?? c.threshold
        case "--misses": c.misses = max(1, Int(it.next() ?? "") ?? c.misses)
        case "--interval": c.interval = max(1, TimeInterval(it.next() ?? "") ?? c.interval)
        case "--verbose", "-v": c.verbose = true
        default:
            FileHandle.standardError.write(Data("未知參數：\(a)\n".utf8))
            exit(2)
        }
    }
    return c
}

// MARK: - 裝置

func pairedDevices() -> [IOBluetoothDevice] {
    (IOBluetoothDevice.pairedDevices() as? [IOBluetoothDevice]) ?? []
}

func findDevice(_ query: String) -> IOBluetoothDevice? {
    let q = query.lowercased().replacingOccurrences(of: ":", with: "-")
    return pairedDevices().first {
        ($0.name ?? "").lowercased() == q
            || ($0.addressString ?? "").lowercased() == q
    } ?? pairedDevices().first {
        ($0.name ?? "").lowercased().contains(q)
    }
}

func log(_ s: String) {
    let f = DateFormatter(); f.dateFormat = "HH:mm:ss"
    print("[\(f.string(from: Date()))] \(s)")
    fflush(stdout)
}

// MARK: - 鎖定

func lockScreen() {
    // 讓顯示器休眠；搭配「需要密碼：立即」即等同鎖定。
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/usr/bin/pmset")
    p.arguments = ["displaysleepnow"]
    try? p.run()
    p.waitUntilExit()
}

// MARK: - 主程式

let cfg = parseArgs()

if cfg.list {
    let devs = pairedDevices()
    if devs.isEmpty { print("沒有已配對的藍牙裝置。") }
    for d in devs {
        print("\(d.name ?? "(未命名)")\t\(d.addressString ?? "?")\t\(d.isConnected() ? "已連線" : "未連線")")
    }
    exit(0)
}

guard let query = cfg.device else {
    print("用法：proximity-lock --list | --device <名稱或位址> [--threshold -80] [--misses 3] [--interval 5] [-v]")
    exit(2)
}
guard let dev = findDevice(query) else {
    print("找不到已配對的裝置「\(query)」。請先用 --list 查看。")
    exit(1)
}

log("監控：\(dev.name ?? "?")（\(dev.addressString ?? "?")），門檻 \(cfg.threshold) dBm，連續 \(cfg.misses) 次，每 \(Int(cfg.interval)) 秒")

var missCount = 0
var locked = false
var connecting = false

func sample() {
    var present = false
    var detail = ""

    if dev.isConnected() {
        let rssi = Int(dev.rawRSSI())   // 127 代表讀不到
        if rssi == 127 || rssi == 0 {
            detail = "已連線，RSSI 無法讀取"
            present = true               // 連線中但沒讀數：保守視為在附近
        } else {
            detail = "RSSI \(rssi) dBm"
            present = rssi >= cfg.threshold
        }
    } else {
        detail = "未連線"
        // 嘗試重新連線（在背景執行，避免卡住主迴圈）
        if !connecting {
            connecting = true
            DispatchQueue.global().async {
                _ = dev.openConnection()
                connecting = false
            }
        }
    }

    if present {
        if locked { log("裝置回到範圍內（\(detail)）") }
        missCount = 0
        locked = false
    } else {
        missCount += 1
        if cfg.verbose || missCount == 1 { log("離開？\(detail)（\(missCount)/\(cfg.misses)）") }
        if missCount >= cfg.misses && !locked {
            log("鎖定螢幕")
            lockScreen()
            locked = true
        }
    }
    if cfg.verbose && present { log("在附近：\(detail)") }
}

let timer = Timer.scheduledTimer(withTimeInterval: cfg.interval, repeats: true) { _ in sample() }
timer.fire()
RunLoop.main.run()
