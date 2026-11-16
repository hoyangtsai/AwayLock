import AppKit
import SwiftUI

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Core.shared.start()   // 啟動 Rust 核心：藍牙掃描與鎖定判定（首次會要求藍牙權限）
        setUpStatusItem()
        showSettings()        // 剛啟動時顯示視窗
    }

    // 關閉視窗後 App 繼續留在狀態列
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    private func setUpStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = statusItem.button {
            let image = NSImage(systemSymbolName: "lock.shield",
                                accessibilityDescription: "AwayLock")
            image?.isTemplate = true   // 隨選單列明暗自動變色
            button.image = image
        }

        let menu = NSMenu()
        let settings = NSMenuItem(title: "Settings", action: #selector(showSettings), keyEquivalent: "")
        settings.target = self
        menu.addItem(settings)
        menu.addItem(NSMenuItem(title: "Quit",
                                action: #selector(NSApplication.terminate(_:)),
                                keyEquivalent: ""))
        statusItem.menu = menu   // 左鍵點擊即顯示選單
    }

    @objc private func showSettings() {
        if window == nil {
            let w = NSWindow(contentViewController: SettingsTabViewController())
            w.styleMask = [.titled, .closable]
            w.title = SettingsTabViewController.windowTitle
            w.isReleasedWhenClosed = false
            w.center()
            window = w
        }
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }
}
