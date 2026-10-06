// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "DeskSessions",
    platforms: [.macOS("26.0")],
    products: [
        .executable(name: "DeskSessions", targets: ["DeskSessions"]),
        .executable(name: "desksessions-hook", targets: ["desksessions-hook"]),
    ],
    targets: [
        // Everything testable without a window or a radio: sources, tracking, protocol, hooks, jumps.
        .target(name: "DeskCore"),
        // The menu-bar app: panel, settings, Bluetooth.
        .executableTarget(name: "DeskSessions", dependencies: ["DeskCore"]),
        // Runs on every Codex hook: forwards the event to the app and exits.
        .executableTarget(name: "desksessions-hook"),
        .testTarget(name: "DeskCoreTests", dependencies: ["DeskCore"]),
    ]
)
