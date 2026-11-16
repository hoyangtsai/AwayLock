import Foundation

struct AllowedDevice: Codable, Identifiable, Hashable {
    let id: UUID          // CoreBluetooth 為該裝置分配的識別碼
    var name: String
}

/// 允許列表：只有列表中的裝置才會影響自動鎖定。存在 UserDefaults。
final class AllowList: ObservableObject {
    static let shared = AllowList()

    @Published private(set) var devices: [AllowedDevice] = []
    private let key = "allowedDevices"

    private init() {
        if let data = UserDefaults.standard.data(forKey: key),
           let list = try? JSONDecoder().decode([AllowedDevice].self, from: data) {
            devices = list
        }
    }

    func contains(_ id: UUID) -> Bool {
        devices.contains { $0.id == id }
    }

    func add(id: UUID, name: String) {
        guard !contains(id) else { return }
        devices.append(AllowedDevice(id: id, name: name))
        save()
    }

    func remove(id: UUID) {
        devices.removeAll { $0.id == id }
        save()
    }

    private func save() {
        if let data = try? JSONEncoder().encode(devices) {
            UserDefaults.standard.set(data, forKey: key)
        }
    }
}
