import Foundation

/// One entry of Claude Code's live-session registry (~/.claude/sessions/<pid>.json).
public struct ClaudeRecord: Sendable, Equatable {
    public var pid: Int32
    public var sessionId: String
    public var cwd: String
    public var name: String
    public var status: String
    public var waitingFor: String?
    public var kind: String

    public init(pid: Int32, sessionId: String, cwd: String, name: String, status: String,
                waitingFor: String? = nil, kind: String = "interactive") {
        self.pid = pid
        self.sessionId = sessionId
        self.cwd = cwd
        self.name = name
        self.status = status
        self.waitingFor = waitingFor
        self.kind = kind
    }
}

public enum ClaudeSessions {
    public static var directory: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions")
    }

    /// Reads every registry file. Unreadable or half-written files are skipped.
    public static func read(from directory: URL = directory) -> [ClaudeRecord] {
        let files = (try? FileManager.default.contentsOfDirectory(
            at: directory, includingPropertiesForKeys: nil)) ?? []
        return files.filter { $0.pathExtension == "json" }.compactMap(parse)
    }

    static func parse(_ url: URL) -> ClaudeRecord? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return parse(data)
    }

    public static func parse(_ data: Data) -> ClaudeRecord? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let pid = json["pid"] as? Int,
              let sessionId = json["sessionId"] as? String,
              let status = json["status"] as? String
        else { return nil }
        let cwd = json["cwd"] as? String ?? ""
        return ClaudeRecord(
            pid: Int32(pid), sessionId: sessionId, cwd: cwd,
            name: json["name"] as? String ?? URL(fileURLWithPath: cwd).lastPathComponent,
            status: status, waitingFor: json["waitingFor"] as? String,
            kind: json["kind"] as? String ?? "interactive")
    }
}
