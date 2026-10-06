import Foundation

/// Adds and removes our entries in ~/.codex/hooks.json, leaving everyone else's alone.
public struct HooksInstaller: Sendable {
    public static let events = [
        "SessionStart", "SessionEnd", "UserPromptSubmit", "PreToolUse", "PostToolUse",
        "PermissionRequest", "Stop", "Interrupt", "SubagentStart", "SubagentStop",
    ]
    static let marker = "desksessions-hook"

    public var file: URL

    public init(file: URL = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".codex/hooks.json")) {
        self.file = file
    }

    public enum Status: Equatable, Sendable {
        case notInstalled
        case installed(hookPath: String)
    }

    public func status() -> Status {
        guard let hooks = (try? load())?["hooks"] as? [String: Any] else { return .notInstalled }
        for groups in hooks.values.compactMap({ $0 as? [[String: Any]] }) {
            for group in groups {
                for hook in group["hooks"] as? [[String: Any]] ?? [] {
                    if let command = hook["command"] as? String, command.contains(Self.marker) {
                        return .installed(hookPath: command.trimmingCharacters(in: CharacterSet(charactersIn: "'")))
                    }
                }
            }
        }
        return .notInstalled
    }

    /// Installs (or repoints) our hook for every event. Backs the file up first, once per change.
    public func install(hookPath: String) throws {
        var root = try load() ?? [:]
        var hooks = root["hooks"] as? [String: Any] ?? [:]
        for event in Self.events {
            var groups = Self.withoutOurs(hooks[event] as? [[String: Any]] ?? [])
            groups.append([
                "matcher": ".*",
                "hooks": [["type": "command", "command": "'\(hookPath)'", "timeout": 2]],
            ])
            hooks[event] = groups
        }
        root["hooks"] = hooks
        try save(root)
    }

    public func uninstall() throws {
        guard var root = try load(), var hooks = root["hooks"] as? [String: Any] else { return }
        for (event, value) in hooks {
            let groups = Self.withoutOurs(value as? [[String: Any]] ?? [])
            hooks[event] = groups.isEmpty ? nil : groups
        }
        root["hooks"] = hooks
        try save(root)
    }

    static func withoutOurs(_ groups: [[String: Any]]) -> [[String: Any]] {
        groups.compactMap { group in
            var group = group
            let hooks = (group["hooks"] as? [[String: Any]] ?? []).filter {
                !(($0["command"] as? String)?.contains(marker) ?? false)
            }
            if hooks.isEmpty { return nil }
            group["hooks"] = hooks
            return group
        }
    }

    private func load() throws -> [String: Any]? {
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        let data = try Data(contentsOf: file)
        return try JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    private func save(_ root: [String: Any]) throws {
        let fm = FileManager.default
        if fm.fileExists(atPath: file.path) {
            let stamp = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
            try? fm.copyItem(at: file, to: file.appendingPathExtension("bak-\(stamp)"))
        } else {
            try fm.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        }
        let data = try JSONSerialization.data(
            withJSONObject: root, options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes])
        try data.write(to: file, options: .atomic)
    }
}
