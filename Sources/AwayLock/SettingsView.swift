import SwiftUI
import ServiceManagement

// 各分頁的尺寸（寬度一致，高度依內容）
enum TabSize {
    static let general = NSSize(width: 560, height: 360)
    static let devices = NSSize(width: 560, height: 440)
    static let debug   = NSSize(width: 560, height: 520)
    static let about   = NSSize(width: 560, height: 260)
}

// MARK: - General

struct GeneralView: View {
    @State private var launchAtLogin = (SMAppService.mainApp.status == .enabled)
    @State private var errorMessage: String?
    @ObservedObject private var settings = AppSettings.shared

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 20) {
                option(title: "開機時啟動",
                       isOn: $launchAtLogin,
                       caption: "Mac 開機時自動開啟 AwayLock。")
                    .onChange(of: launchAtLogin) { enabled in apply(enabled) }

                if let errorMessage {
                    Text(errorMessage).font(.caption).foregroundStyle(.red)
                }

                thresholdSection

                #if DEBUG
                option(title: "Debug 模式",
                       isOn: $settings.debugMode,
                       caption: "僅開發版：顯示 Debug 分頁，列出掃描與判定細節。")
                #endif
            }
            .frame(width: 400, alignment: .leading)
            .padding(.top, 24)

            Spacer(minLength: 12)

            Divider().padding(.horizontal, 40)
            HStack {
                Spacer()
                Button("Quit") { NSApp.terminate(nil) }
            }
            .padding(.horizontal, 40)
            .padding(.vertical, 16)
        }
        .frame(width: TabSize.general.width, height: TabSize.general.height)
    }

    private func option(title: String, isOn: Binding<Bool>, caption: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Toggle(title, isOn: isOn)
            Text(caption)
                .font(.callout)
                .foregroundStyle(.secondary)
                .padding(.leading, 22)
        }
    }

    private var thresholdSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("離開距離：約 \(String(format: "%.1f", settings.distanceMeters)) 公尺")
            Slider(value: Binding(get: { settings.distanceMeters },
                                  set: { settings.distanceMeters = ($0 * 2).rounded() / 2 }),
                   in: 0.5...5, step: 0.5) {
                EmptyView()
            } minimumValueLabel: {
                Text("近").font(.caption)
            } maximumValueLabel: {
                Text("遠").font(.caption)
            }
            Text("電腦解鎖期間掃描到過的允許裝置，只要有任何一個超過這個距離或突然掃描不到，就會自動鎖定螢幕。距離是由藍牙訊號粗估，會受牆壁和遮擋影響，僅供參考。")
                .font(.callout)
                .foregroundStyle(.secondary)
        }
    }

    private func apply(_ enabled: Bool) {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            errorMessage = nil
        } catch {
            errorMessage = "無法變更：\(error.localizedDescription)"
            launchAtLogin = (SMAppService.mainApp.status == .enabled)
        }
    }
}

// MARK: - Devices

struct DevicesView: View {
    @ObservedObject private var scanner = BLEScanner.shared
    @ObservedObject private var allowList = AllowList.shared

    private var candidates: [NearbyDevice] {
        scanner.nearby.filter { !allowList.contains($0.id) }
    }

    var body: some View {
        List {
            Section {
                if allowList.devices.isEmpty {
                    Text("尚未加入任何裝置。加入後，只有這些裝置離開時才會鎖定。")
                        .foregroundStyle(.secondary)
                }
                ForEach(allowList.devices) { a in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(a.name)
                            Text(statusText(for: a))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("移除") { allowList.remove(id: a.id) }
                    }
                }
            } header: {
                Text("允許的裝置")
            }

            Section {
                if !scanner.statusText.isEmpty && scanner.statusText != "掃描中" {
                    Text(scanner.statusText).foregroundStyle(.orange)
                }
                ForEach(candidates) { d in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(d.displayName)
                            Text("約 \(formatMeters(estimatedMeters(rssi: d.rssi)))" + (d.isApple ? " · Apple" : ""))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("加入") {
                            allowList.add(id: d.id, name: d.displayName)
                        }
                    }
                }
            } header: {
                Text("附近的裝置（\(scanner.statusText)）")
            } footer: {
                Text("提示：把手機或手錶靠近 Mac，距離最近的通常就是它。Apple 裝置的識別碼會定期更換，更換後需要重新加入。")
                    .font(.caption)
            }
        }
        .frame(width: TabSize.devices.width, height: TabSize.devices.height)
    }

    private func statusText(for a: AllowedDevice) -> String {
        guard let d = scanner.device(a.id),
              Date().timeIntervalSince(d.lastSeen) < 15 else { return "未偵測到" }
        return "在附近 · 約 \(formatMeters(estimatedMeters(rssi: d.rssi)))"
    }
}

// MARK: - Debug（僅開發版）

#if DEBUG
struct DebugView: View {
    @ObservedObject private var scanner = BLEScanner.shared
    @ObservedObject private var allowList = AllowList.shared
    @ObservedObject private var monitor = ProximityMonitor.shared
    @ObservedObject private var settings = AppSettings.shared

    private static let timeFormat: DateFormatter = {
        let f = DateFormatter(); f.dateFormat = "HH:mm:ss"; return f
    }()

    var body: some View {
        List {
            Section("離開判定") {
                row("離開距離（可在 General 調整）",
                    "\(String(format: "%.1f", settings.distanceMeters)) 公尺 ≈ \(settings.threshold) dBm")
                row("Mac 藍牙", scanner.statusText)
                row("允許的裝置數", "\(allowList.devices.count)")
                row("電腦狀態", monitor.isLocked ? "已鎖定" : "已解鎖")
                row("本次解鎖期間見過的允許裝置", "\(monitor.armedIDs.count) / \(allowList.devices.count)")
                row("連續離開次數（監控中裝置）", "\(monitor.missCount) / \(monitor.requiredMisses)")
                row("最近檢查", monitor.lastCheck.map { Self.timeFormat.string(from: $0) } ?? "—")
                row("判定", monitor.decision)
                row("檢查間隔 / 失聯判定", "\(Int(monitor.interval)) 秒 / \(Int(monitor.staleAfter)) 秒")
            }

            Section("允許的裝置（即時）") {
                if allowList.devices.isEmpty {
                    Text("無").foregroundStyle(.secondary)
                }
                ForEach(allowList.devices) { a in
                    let result = monitor.evaluate(a)
                    let d = scanner.device(a.id)
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(a.name)
                            Spacer()
                            Text(result.present ? "在範圍內" : "離開")
                                .foregroundStyle(result.present ? .green : .red)
                        }
                        Text(detailLine(d) + " · " + result.reason
                             + (monitor.armedIDs.contains(a.id) ? " · 監控中" : " · 尚未見過"))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                        Text(a.id.uuidString)
                            .font(.caption2.monospaced())
                            .foregroundStyle(.tertiary)
                    }
                }
            }

            Section("掃描到的所有裝置（\(scanner.nearby.count)）") {
                ForEach(scanner.nearby) { d in
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(d.displayName)
                            if allowList.contains(d.id) {
                                Image(systemName: "checkmark.seal.fill").foregroundStyle(.green)
                            }
                            Spacer()
                            Text(d.isApple ? "Apple" : "").foregroundStyle(.secondary)
                        }
                        Text(detailLine(d))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                        Text(d.id.uuidString)
                            .font(.caption2.monospaced())
                            .foregroundStyle(.tertiary)
                    }
                }
            }
        }
        .frame(width: TabSize.debug.width, height: TabSize.debug.height)
    }

    private func detailLine(_ d: NearbyDevice?) -> String {
        guard let d else { return "無資料" }
        let age = Int(Date().timeIntervalSince(d.lastSeen))
        return "\(d.rssi) dBm · 約 \(formatMeters(estimatedMeters(rssi: d.rssi))) · \(age) 秒前"
    }

    private func row(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title)
            Spacer()
            Text(value).foregroundStyle(.secondary).monospacedDigit()
        }
    }
}
#endif

// MARK: - About

struct AboutView: View {
    private var version: String {
        let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
        return "v\(v)"
    }

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "lock.shield")
                .font(.system(size: 40))
            Text("AwayLock").font(.title2).bold()
            Text(version).foregroundStyle(.secondary)
            Text("iPhone / Apple Watch 離開藍牙範圍時，自動鎖定 Mac。")
                .font(.callout)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .padding(.top, 4)
        }
        .padding(24)
        .frame(width: TabSize.about.width, height: TabSize.about.height)
    }
}
