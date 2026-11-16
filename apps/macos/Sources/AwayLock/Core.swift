import Foundation
import CAwayLock

// MARK: - Rust 核心回傳的資料（JSON）

struct AllowedInfo: Decodable, Identifiable {
    let id: String
    let name: String
    let seen: Bool
    let distanceM: Double?
}

struct NearbyInfo: Decodable, Identifiable {
    let id: String
    let name: String
    let isApple: Bool
    let distanceM: Double
}

struct CoreState: Decodable {
    var distanceM: Double
    var bluetoothStatus: String
    var bluetoothReady: Bool
    var allowed: [AllowedInfo]
    var nearby: [NearbyInfo]

    static let empty = CoreState(distanceM: 5.0, bluetoothStatus: "初始化中…",
                                 bluetoothReady: false, allowed: [], nearby: [])
}

#if DEBUG
struct DebugAllowed: Decodable, Identifiable {
    let id: String
    let name: String
    let present: Bool
    let reason: String
    let armed: Bool
    let rssi: Int?
    let distanceM: Double?
    let ageSecs: Int?
}

struct DebugDevice: Decodable, Identifiable {
    let id: String
    let name: String
    let isApple: Bool
    let allowed: Bool
    let rssi: Int
    let distanceM: Double
    let ageSecs: Int
}

struct DebugState: Decodable {
    let thresholdDbm: Int
    let distanceM: Double
    let bluetoothStatus: String
    let phase: String
    let armedCount: Int
    let allowedCount: Int
    let maxMisses: Int
    let requiredMisses: Int
    let lastCheckSecs: Int?
    let decision: String
    let tickSecs: Int
    let staleSecs: Int
    let allowed: [DebugAllowed]
    let all: [DebugDevice]
}
#endif

func formatMeters(_ m: Double) -> String {
    m < 10 ? String(format: "%.1f 公尺", m) : String(format: "%.0f 公尺", m)
}

// MARK: - 與 Rust 核心的橋接

/// 啟動 Rust 核心，並每秒更新一次畫面要用的狀態。
final class Core: ObservableObject {
    static let shared = Core()

    @Published private(set) var state = CoreState.empty
    #if DEBUG
    @Published private(set) var debug: DebugState?
    #endif

    private var timer: Timer?
    private let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }()

    private init() {}

    func start() {
        awaylock_start()
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.refresh()
        }
    }

    func setDistance(_ meters: Double) {
        awaylock_set_distance(meters)
        refresh()
    }

    func add(id: String, name: String) {
        awaylock_add_device(id, name)
        refresh()
    }

    func remove(id: String) {
        awaylock_remove_device(id)
        refresh()
    }

    private func refresh() {
        if let data = take(awaylock_state_json()),
           let s = try? decoder.decode(CoreState.self, from: data) {
            state = s
        }
        #if DEBUG
        if let data = take(awaylock_debug_json()),
           let d = try? decoder.decode(DebugState.self, from: data) {
            debug = d
        }
        #endif
    }

    /// 取出 Rust 回傳的字串並釋放。
    private func take(_ p: UnsafeMutablePointer<CChar>?) -> Data? {
        guard let p else { return nil }
        defer { awaylock_free_string(p) }
        return String(cString: p).data(using: .utf8)
    }
}
