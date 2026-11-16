import SwiftUI
import ServiceManagement

// 各分頁的尺寸（寬度一致，高度依內容）
enum TabSize {
    static let general = NSSize(width: 560, height: 360)
    static let devices = NSSize(width: 560, height: 440)
    static let debug   = NSSize(width: 560, height: 520)
    static let about   = NSSize(width: 560, height: 260)
}

// MARK: - 共用

// MARK: - General

struct GeneralView: View {
    @State private var launchAtLogin = (SMAppService.mainApp.status == .enabled)
    @State private var errorMessage: String?
    @ObservedObject private var core = Core.shared
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
            Text("離開距離：約 \(String(format: "%.1f", core.state.distanceM)) 公尺")
            Slider(value: Binding(get: { core.state.distanceM },
                                  set: { core.setDistance($0) }),
                   in: 1...5, step: 0.5) {
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
    @ObservedObject private var core = Core.shared
    #if DEBUG
    @ObservedObject private var settings = AppSettings.shared
    #endif

    // 版面與 General 相同：上方是固定的 VStack，只有「附近的裝置」在內部捲動，
    // 捲動區不會延伸到標題列下面，所以標題列顏色和 General 一致。
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionTitle("允許的裝置")
            if core.state.allowed.isEmpty {
                Text("尚未加入任何裝置。加入後，只有這些裝置離開時才會鎖定。")
                    .foregroundStyle(.secondary)
            }
            ForEach(core.state.allowed) { a in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(a.name)
                        Text(statusText(for: a))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("移除") { core.remove(id: a.id) }
                }
            }

            Divider().padding(.vertical, 4)

            sectionTitle("附近的裝置（\(core.state.bluetoothStatus)）")
            #if DEBUG
            // 僅開發版且開啟 Debug 模式時，才能選擇顯示全部裝置
            if settings.debugMode {
                Toggle("只顯示手機與手錶（關閉＝顯示全部裝置）", isOn: Binding(
                    get: { core.state.onlyPhonesWatches },
                    set: { core.setOnlyPhonesWatches($0) }))
                if core.state.onlyPhonesWatches && core.state.hiddenCount > 0 {
                    Text("已隱藏 \(core.state.hiddenCount) 個其他裝置。")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            #endif
            if !core.state.bluetoothReady {
                Text(core.state.bluetoothStatus).foregroundStyle(.orange)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(core.state.nearby) { d in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(d.name)
                                Text("約 \(formatMeters(d.distanceM))" + (d.isApple ? " · Apple" : ""))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("加入") { core.add(id: d.id, name: d.name) }
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            Text("這裡只列出名稱顯示為手機或手錶的裝置。把手機或手錶靠近 Mac，距離最近的通常就是它。Apple 裝置的識別碼會定期更換，更換後需要重新加入。")
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 32)
        .padding(.top, 24)
        .padding(.bottom, 16)
        .frame(width: TabSize.devices.width, height: TabSize.devices.height, alignment: .topLeading)
    }

    private func sectionTitle(_ text: String) -> some View {
        Text(text)
            .font(.subheadline.weight(.semibold))
            .foregroundStyle(.secondary)
    }

    private func statusText(for a: AllowedInfo) -> String {
        guard a.seen, let m = a.distanceM else { return "未偵測到" }
        return "在附近 · 約 \(formatMeters(m))"
    }
}

// MARK: - Debug（僅開發版）

#if DEBUG
struct DebugView: View {
    @ObservedObject private var core = Core.shared

    var body: some View {
        VStack(spacing: 0) {
        Text("除錯資訊（僅開發版）")
            .font(.subheadline.weight(.semibold))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 32)
            .padding(.vertical, 12)
        List {
            if let d = core.debug {
                Section("離開判定") {
                    row("離開距離（可在 General 調整）",
                        "\(String(format: "%.1f", d.distanceM)) 公尺 ≈ \(d.thresholdDbm) dBm")
                    row("Mac 藍牙", d.bluetoothStatus)
                    row("電腦狀態", d.phase)
                    row("本次解鎖期間見過的允許裝置", "\(d.armedCount) / \(d.allowedCount)")
                    row("連續離開次數（監控中裝置）", "\(d.maxMisses) / \(d.requiredMisses)")
                    row("最近檢查", d.lastCheckSecs.map { "\($0) 秒前" } ?? "—")
                    row("判定", d.decision)
                    row("檢查間隔 / 失聯判定", "\(d.tickSecs) 秒 / \(d.staleSecs) 秒")
                }

                Section("允許的裝置（即時）") {
                    if d.allowed.isEmpty {
                        Text("無").foregroundStyle(.secondary)
                    }
                    ForEach(d.allowed) { a in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(a.name)
                                Spacer()
                                Text(a.present ? "在範圍內" : "離開")
                                    .foregroundStyle(a.present ? .green : .red)
                            }
                            Text(allowedLine(a))
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                            Text(a.id)
                                .font(.caption2.monospaced())
                                .foregroundStyle(.tertiary)
                        }
                    }
                }

                Section("掃描到的所有裝置（\(d.all.count)）") {
                    ForEach(d.all) { x in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(x.name)
                                if x.allowed {
                                    Image(systemName: "checkmark.seal.fill").foregroundStyle(.green)
                                }
                                Spacer()
                                Text((x.isApple ? "Apple · " : "") + x.kind).foregroundStyle(.secondary)
                            }
                            Text("\(x.rssi) dBm · 約 \(formatMeters(x.distanceM)) · \(x.ageSecs) 秒前"
                                 + (x.appleTypes.isEmpty ? "" : " · 類型 \(x.appleTypes)"))
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                            Text(x.id)
                                .font(.caption2.monospaced())
                                .foregroundStyle(.tertiary)
                        }
                    }
                }
            } else {
                Text("尚無資料").foregroundStyle(.secondary)
            }
        }
        .scrollContentBackground(.hidden)
        }
        .frame(width: TabSize.debug.width, height: TabSize.debug.height)
    }

    private func allowedLine(_ a: DebugAllowed) -> String {
        var parts: [String] = []
        if let rssi = a.rssi, let m = a.distanceM, let age = a.ageSecs {
            parts.append("\(rssi) dBm · 約 \(formatMeters(m)) · \(age) 秒前")
        } else {
            parts.append("無資料")
        }
        parts.append(a.reason)
        parts.append(a.armed ? "監控中" : "尚未見過")
        return parts.joined(separator: " · ")
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
