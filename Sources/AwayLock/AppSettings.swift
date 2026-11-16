import Foundation

/// 使用者可調整的設定，存在 UserDefaults。
final class AppSettings: ObservableObject {
    static let shared = AppSettings()

    /// 使用者設定的「離開距離」（公尺）：超過這個距離就視為離開。
    /// 以 0.5 公尺為一個間隔。
    @Published var distanceMeters: Double {
        didSet { UserDefaults.standard.set(distanceMeters, forKey: "awayDistanceMeters") }
    }

    /// 對應的 RSSI 門檻（dBm），內部判定與 Debug 使用，不顯示給一般使用者。
    var threshold: Int {
        Int((-59.0 - 25.0 * log10(max(0.1, distanceMeters))).rounded())
    }

    #if DEBUG
    /// 僅開發版（debug 編譯）存在；發布版完全沒有這個選項。
    @Published var debugMode: Bool {
        didSet { UserDefaults.standard.set(debugMode, forKey: "debugMode") }
    }
    #endif

    private init() {
        let d = (UserDefaults.standard.object(forKey: "awayDistanceMeters") as? NSNumber)?.doubleValue
        // 範圍 0.5～5 公尺，以 0.5 公尺為一格（舊設定值自動對齊）
        distanceMeters = min(5.0, max(0.5, ((d ?? 5.0) * 2).rounded() / 2))
        #if DEBUG
        debugMode = UserDefaults.standard.bool(forKey: "debugMode")
        #endif
    }
}

/// 由 RSSI 粗估距離（公尺）。使用對數距離路徑損耗模型：
/// 假設 1 公尺處約 -59 dBm、環境係數 2.5。只是估算，牆壁、身體、
/// 手機擺放位置都會讓誤差很大。
func estimatedMeters(rssi: Int) -> Double {
    pow(10, (-59.0 - Double(rssi)) / 25.0)
}

func formatMeters(_ m: Double) -> String {
    m < 10 ? String(format: "%.1f 公尺", m) : String(format: "%.0f 公尺", m)
}
