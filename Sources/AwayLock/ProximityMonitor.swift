import Foundation

/// 鎖定規則：
/// 1. 電腦每次解鎖後開始一段新的「解鎖期間」。
/// 2. 在這段期間內，允許列表中的裝置只要被掃描到且在設定距離內，就「列入監控」。
/// 3. 列入監控的裝置中，只要有任何一個超過設定距離，或突然掃描不到，就鎖定螢幕。
///    從未在這段期間被掃描到的裝置不會觸發鎖定。
/// 4. 鎖定後清空監控名單；下次解鎖再重新開始。
final class ProximityMonitor: ObservableObject {
    static let shared = ProximityMonitor()

    let staleAfter: TimeInterval = 15   // 超過幾秒沒收到廣播視為掃描不到
    let requiredMisses = 3              // 連續幾次檢查都離開才鎖定（避免訊號瞬間抖動誤鎖）
    let interval: TimeInterval = 5      // 檢查間隔（秒）

    @Published private(set) var armedIDs: Set<UUID> = []   // 本次解鎖期間見過的允許裝置
    @Published private(set) var missCount = 0              // 監控中裝置的最大連續離開次數
    @Published private(set) var isLocked = false           // 電腦目前是否鎖定
    @Published private(set) var lastCheck: Date?
    @Published private(set) var decision = "尚未檢查"

    private var misses: [UUID: Int] = [:]
    private var timer: Timer?

    private init() {}

    func start() {
        timer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            self?.sample()
        }

        // 監聽系統的鎖定／解鎖事件
        let center = DistributedNotificationCenter.default()
        center.addObserver(forName: Notification.Name("com.apple.screenIsLocked"),
                           object: nil, queue: .main) { [weak self] _ in
            self?.enterLocked()
        }
        center.addObserver(forName: Notification.Name("com.apple.screenIsUnlocked"),
                           object: nil, queue: .main) { [weak self] _ in
            self?.beginUnlockedSession()
        }
    }

    // MARK: 狀態切換

    private func enterLocked() {
        isLocked = true
        armedIDs = []
        misses = [:]
        missCount = 0
        decision = "電腦已鎖定，等待解鎖"
    }

    private func beginUnlockedSession() {
        isLocked = false
        armedIDs = []
        misses = [:]
        missCount = 0
        decision = "電腦已解鎖，開始新的監控期間"
    }

    // MARK: 判定

    /// 判斷單一允許裝置目前是否在範圍內，並說明原因（供 Debug 顯示）。
    func evaluate(_ a: AllowedDevice, now: Date = Date()) -> (present: Bool, reason: String) {
        let threshold = AppSettings.shared.threshold
        guard let d = BLEScanner.shared.device(a.id) else { return (false, "未收到廣播") }
        let age = now.timeIntervalSince(d.lastSeen)
        if age >= staleAfter { return (false, "已 \(Int(age)) 秒沒收到廣播") }
        if d.rssi < threshold { return (false, "訊號 \(d.rssi) < 門檻 \(threshold)") }
        return (true, "訊號 \(d.rssi) ≥ 門檻 \(threshold)")
    }

    private func sample() {
        lastCheck = Date()
        guard !isLocked else { decision = "電腦已鎖定，等待解鎖"; return }
        guard BLEScanner.shared.isPoweredOn else { decision = "藍牙不可用，不動作"; return }

        let allowed = AllowList.shared.devices
        guard !allowed.isEmpty else {
            armedIDs = []; misses = [:]; missCount = 0
            decision = "允許列表是空的，不動作"
            return
        }

        // 移除已從允許列表刪除的裝置
        let allowedIDs = Set(allowed.map(\.id))
        armedIDs = armedIDs.intersection(allowedIDs)
        misses = misses.filter { allowedIDs.contains($0.key) }

        let now = Date()
        for a in allowed {
            if evaluate(a, now: now).present {
                armedIDs.insert(a.id)          // 見過了：列入監控
                misses[a.id] = 0
            } else if armedIDs.contains(a.id) {
                misses[a.id, default: 0] += 1  // 監控中的裝置離開了
            }
        }
        missCount = armedIDs.map { misses[$0] ?? 0 }.max() ?? 0

        // 任何一個監控中的裝置連續離開，就鎖定
        if let lost = allowed.first(where: { armedIDs.contains($0.id) && (misses[$0.id] ?? 0) >= requiredMisses }) {
            decision = "「\(lost.name)」離開，已鎖定螢幕"
            lockScreen()
            enterLocked()
            decision = "「\(lost.name)」離開，已鎖定螢幕"
            return
        }

        if armedIDs.isEmpty {
            decision = "本次解鎖期間尚未掃描到允許的裝置，不動作"
        } else if missCount > 0 {
            decision = "有監控中的裝置離開（\(missCount)/\(requiredMisses)）"
        } else {
            decision = "監控中（已見過 \(armedIDs.count) 個允許裝置）"
        }
    }

    private func lockScreen() {
        // 顯示器休眠；搭配「需要密碼：立即」即等同鎖定
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/pmset")
        p.arguments = ["displaysleepnow"]
        try? p.run()
    }
}
