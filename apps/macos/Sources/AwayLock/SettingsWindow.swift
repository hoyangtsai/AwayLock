import AppKit
import SwiftUI
import Combine

/// 工具列樣式的分頁（圖示在上、文字在下），像系統設定視窗。
final class SettingsTabViewController: NSTabViewController {
    static let windowTitle = "AwayLock – Settings"

    #if DEBUG
    private var debugItem: NSTabViewItem?
    private var cancellables = Set<AnyCancellable>()
    #endif

    override func viewDidLoad() {
        super.viewDidLoad()
        tabStyle = .toolbar

        addTabViewItem(makeItem(GeneralView(), title: "General",
                                symbol: "gearshape", size: TabSize.general))
        addTabViewItem(makeItem(DevicesView(), title: "Devices",
                                symbol: "dot.radiowaves.left.and.right", size: TabSize.devices))
        addTabViewItem(makeItem(AboutView(), title: "About",
                                symbol: "info.circle", size: TabSize.about))

        #if DEBUG
        setDebugTab(AppSettings.shared.debugMode)
        AppSettings.shared.$debugMode
            .dropFirst()
            .receive(on: RunLoop.main)
            .sink { [weak self] on in self?.setDebugTab(on) }
            .store(in: &cancellables)
        #endif
    }

    // 視窗標題固定為「AwayLock – Settings」，不隨分頁變動
    override func tabView(_ tabView: NSTabView, didSelect tabViewItem: NSTabViewItem?) {
        super.tabView(tabView, didSelect: tabViewItem)
        DispatchQueue.main.async { [weak self] in
            self?.view.window?.title = Self.windowTitle
        }
    }

    private func makeItem<V: View>(_ view: V, title: String, symbol: String, size: NSSize) -> NSTabViewItem {
        let hc = NSHostingController(rootView: view)
        hc.preferredContentSize = size
        let item = NSTabViewItem(viewController: hc)
        item.label = title
        item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: title)
        return item
    }

    #if DEBUG
    private func setDebugTab(_ on: Bool) {
        if on, debugItem == nil {
            let item = makeItem(DebugView(), title: "Debug",
                                symbol: "ladybug", size: TabSize.debug)
            debugItem = item
            insertTabViewItem(item, at: 2)   // General, Devices, Debug, About
        } else if !on, let item = debugItem {
            removeTabViewItem(item)
            debugItem = nil
        }
    }
    #endif
}
