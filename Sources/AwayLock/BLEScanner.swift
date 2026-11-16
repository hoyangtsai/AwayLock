import Foundation
import CoreBluetooth

struct NearbyDevice: Identifiable, Equatable {
    let id: UUID
    var name: String?
    var rssi: Int            // 平滑後的訊號強度 (dBm)
    var lastSeen: Date
    var isApple: Bool        // 廣播資料的廠商代碼為 Apple (0x004C)

    var displayName: String {
        if let name, !name.isEmpty { return name }
        let short = id.uuidString.prefix(8)
        return (isApple ? "Apple 裝置 " : "未命名裝置 ") + short
    }
}

/// 持續掃描附近的 BLE 廣播。UI 使用 `nearby`（每秒更新一次），
/// 監控邏輯使用 `device(_:)` 取得最新資料。
final class BLEScanner: NSObject, ObservableObject, CBCentralManagerDelegate {
    static let shared = BLEScanner()

    @Published private(set) var nearby: [NearbyDevice] = []
    @Published private(set) var statusText = "初始化中…"
    private(set) var isPoweredOn = false

    private var central: CBCentralManager!
    private var table: [UUID: NearbyDevice] = [:]
    private var publishTimer: Timer?

    private override init() {
        super.init()
        central = CBCentralManager(delegate: self, queue: .main)
        publishTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.publish()
        }
    }

    func device(_ id: UUID) -> NearbyDevice? { table[id] }

    // MARK: CBCentralManagerDelegate

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            isPoweredOn = true
            statusText = "掃描中"
            central.scanForPeripherals(withServices: nil,
                                       options: [CBCentralManagerScanOptionAllowDuplicatesKey: true])
        case .unauthorized:
            isPoweredOn = false
            statusText = "未授權藍牙：請到「系統設定 → 隱私權與安全性 → 藍牙」允許 AwayLock"
        case .poweredOff:
            isPoweredOn = false
            statusText = "藍牙已關閉"
        default:
            isPoweredOn = false
            statusText = "藍牙不可用"
        }
    }

    func centralManager(_ central: CBCentralManager,
                        didDiscover peripheral: CBPeripheral,
                        advertisementData: [String: Any],
                        rssi RSSI: NSNumber) {
        let value = RSSI.intValue
        guard value != 127 else { return }   // 127 代表無效讀數

        let adName = advertisementData[CBAdvertisementDataLocalNameKey] as? String
        var isApple = false
        if let md = advertisementData[CBAdvertisementDataManufacturerDataKey] as? Data,
           md.count >= 2 {
            isApple = (md[0] == 0x4C && md[1] == 0x00)
        }

        let id = peripheral.identifier
        if var existing = table[id] {
            existing.rssi = Int((Double(existing.rssi) * 0.7 + Double(value) * 0.3).rounded())
            existing.lastSeen = Date()
            if let n = peripheral.name ?? adName { existing.name = n }
            existing.isApple = existing.isApple || isApple
            table[id] = existing
        } else {
            table[id] = NearbyDevice(id: id,
                                     name: peripheral.name ?? adName,
                                     rssi: value,
                                     lastSeen: Date(),
                                     isApple: isApple)
        }
    }

    // MARK: 發布給 UI

    private func publish() {
        let cutoff = Date().addingTimeInterval(-30)
        table = table.filter { $0.value.lastSeen > cutoff }
        nearby = table.values.sorted { $0.rssi > $1.rssi }
    }
}
