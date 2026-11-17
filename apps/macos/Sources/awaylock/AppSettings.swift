import Foundation

/// 只屬於介面的設定。離開距離、允許列表等由 Rust 核心保存（見 Core.swift）。
final class AppSettings: ObservableObject {
    static let shared = AppSettings()

    #if DEBUG
    /// 僅開發版（debug 編譯）存在；發布版完全沒有這個選項。
    @Published var debugMode: Bool {
        didSet { UserDefaults.standard.set(debugMode, forKey: "debugMode") }
    }
    #endif

    private init() {
        #if DEBUG
        debugMode = UserDefaults.standard.bool(forKey: "debugMode")
        #endif
    }
}
