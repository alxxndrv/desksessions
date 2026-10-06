import Foundation

/// A Codex hook event as forwarded by desksessions-hook: Codex's own JSON payload plus where
/// the hook ran (its parent process and cmux surface).
public struct CodexEvent: Sendable, Equatable {
    public enum Kind: String, Sendable {
        case sessionStart = "SessionStart"
        case userPromptSubmit = "UserPromptSubmit"
        case preToolUse = "PreToolUse"
        case postToolUse = "PostToolUse"
        case permissionRequest = "PermissionRequest"
        case stop = "Stop"
        case sessionEnd = "SessionEnd"
        case interrupt = "Interrupt"
        case subagentStart = "SubagentStart"
        case subagentStop = "SubagentStop"
    }

    public var kind: Kind
    public var sessionId: String
    public var cwd: String
    public var toolName: String?
    public var agentId: String?
    public var parentPid: Int32?
    public var cmuxWorkspace: String?
    public var cmuxSurface: String?

    public init(kind: Kind, sessionId: String, cwd: String = "", toolName: String? = nil,
                agentId: String? = nil, parentPid: Int32? = nil, cmuxWorkspace: String? = nil,
                cmuxSurface: String? = nil) {
        self.kind = kind
        self.sessionId = sessionId
        self.cwd = cwd
        self.toolName = toolName
        self.agentId = agentId
        self.parentPid = parentPid
        self.cmuxWorkspace = cmuxWorkspace
        self.cmuxSurface = cmuxSurface
    }

    /// Parses one line from the hook socket: `{"payload": {…}, "env": {…}, "ppid": n}`.
    public static func parse(line: Data) -> CodexEvent? {
        guard let json = try? JSONSerialization.jsonObject(with: line) as? [String: Any],
              let payload = json["payload"] as? [String: Any],
              let name = payload["hook_event_name"] as? String,
              let kind = Kind(rawValue: name),
              let sessionId = payload["session_id"] as? String
        else { return nil }
        let env = json["env"] as? [String: String] ?? [:]
        return CodexEvent(
            kind: kind, sessionId: sessionId, cwd: payload["cwd"] as? String ?? "",
            toolName: payload["tool_name"] as? String, agentId: payload["agent_id"] as? String,
            parentPid: (json["ppid"] as? Int).map(Int32.init),
            cmuxWorkspace: env["CMUX_WORKSPACE_ID"], cmuxSurface: env["CMUX_SURFACE_ID"])
    }
}
