// swift-tools-version:5.9
//
// macOS 原生介面（SwiftUI / AppKit）。偵測與判定的邏輯在 Rust 核心，
// 透過 CAwayLock 的 C 介面呼叫。連結 libawaylock_ffi 的參數由 scripts/bundle-macos.sh 傳入。
import PackageDescription

let package = Package(
    name: "AwayLock",
    platforms: [.macOS(.v13)],
    targets: [
        .target(
            name: "CAwayLock",
            path: "Sources/CAwayLock"
        ),
        .executableTarget(
            name: "AwayLock",
            dependencies: ["CAwayLock"],
            path: "Sources/AwayLock"
        ),
    ]
)
