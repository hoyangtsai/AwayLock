// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "AwayLock",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "AwayLock",
            path: "Sources/AwayLock"
        )
    ]
)
