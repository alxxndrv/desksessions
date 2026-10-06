import Foundation

public enum Agent: UInt8, Sendable, Codable {
    case claude = 0
    case codex = 1
}

/// Ordered by urgency, as on the clock.
public enum SessionState: UInt8, Sendable, Codable, Comparable {
    case needsYou = 0
    case done = 1
    case working = 2

    public static func < (a: SessionState, b: SessionState) -> Bool { a.rawValue < b.rawValue }
}

/// Where a session lives, for jumping to it.
public enum SessionHome: Sendable, Equatable {
    case cmux(workspace: String?, surface: String)
    case codexDesktop
    case unknown
}

/// A session as the clock and the panel show it.
public struct Session: Sendable, Equatable, Identifiable {
    public var id: UInt32
    public var agent: Agent
    public var state: SessionState
    public var name: String
    public var detail: String
    public var subagents: UInt8
    public var stateSince: Date
    public var cwd: String
    public var home: SessionHome

    public init(id: UInt32, agent: Agent, state: SessionState, name: String, detail: String = "",
                subagents: UInt8 = 0, stateSince: Date, cwd: String = "", home: SessionHome = .unknown) {
        self.id = id
        self.agent = agent
        self.state = state
        self.name = name
        self.detail = detail
        self.subagents = subagents
        self.stateSince = stateSince
        self.cwd = cwd
        self.home = home
    }
}

extension Session {
    /// NEEDS YOU → DONE → WORKING, longest-waiting first, then id for stability.
    public static func boardOrder(_ a: Session, _ b: Session) -> Bool {
        if a.state != b.state { return a.state < b.state }
        if a.stateSince != b.stateSince { return a.stateSince < b.stateSince }
        return a.id < b.id
    }
}

/// Stable 32-bit id for a session key (FNV-1a), the same on every run.
public func sessionID(_ key: String) -> UInt32 {
    var hash: UInt32 = 2_166_136_261
    for byte in key.utf8 {
        hash ^= UInt32(byte)
        hash = hash &* 16_777_619
    }
    return hash
}
