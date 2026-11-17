// swift-tools-version:5.9
//
// macOS 原生介面（SwiftUI / AppKit）。偵測與判定的邏輯在 Rust 核心，
// 透過 cawaylock 的 C 介面呼叫。連結 libawaylock_ffi 的參數由 scripts/bundle-macos.sh 傳入。
import PackageDescription

let package = Package(
    name: "awaylock",
    platforms: [.macOS(.v13)],
    targets: [
        .target(
            name: "cawaylock",
            path: "Sources/cawaylock"
        ),
        .executableTarget(
            name: "awaylock",
            dependencies: ["cawaylock"],
            path: "Sources/awaylock"
        ),
    ]
)
