import Foundation

/// Every session the Mac knows about and what it is doing, fed by Claude's registry and Codex's
/// hooks. Decides DONE (a turn finished and you haven't been back), dismissals and the daily sweep.
public struct SessionTracker: Sendable {
    public static let doneSweep: TimeInterval = 24 * 3600
    /// A Codex session with no process to watch (Codex Desktop) goes quiet after this long.
    public static let orphanTimeout: TimeInterval = 2 * 3600

    enum Phase: Sendable, Equatable {
        case hidden  // idle and already seen
        case working
        case needsYou
        case done
    }

    struct Entry: Sendable {
        var session: Session
        var phase: Phase
        var pid: Int32?
        var lastEvent: Date
    }

    public var filter: SessionFilter
    private(set) var entries: [String: Entry] = [:]

    public init(filter: SessionFilter = SessionFilter()) {
        self.filter = filter
    }

    // MARK: Claude Code

    /// Replaces what we know about Claude sessions with the registry's current contents.
    public mutating func updateClaude(_ records: [ClaudeRecord], now: Date,
                                      isAlive: (Int32) -> Bool, home: (Int32) -> SessionHome) {
        var seen = Set<String>()
        for record in records where record.kind == "interactive" && isAlive(record.pid) {
            let key = "claude:\(record.sessionId)"
            seen.insert(key)
            let reported: Phase?
            switch record.status {
            // "shell" means a background command (a dev server, say) is running while Claude
            // itself sits idle, sometimes for days: not work in progress.
            case "busy": reported = .working
            case "waiting": reported = .needsYou
            default: reported = nil  // idle
            }
            let detail = reported == .needsYou ? Self.waitingDetail(record.waitingFor) : ""

            var entry = entries[key] ?? Entry(
                session: Session(id: sessionID(key), agent: .claude, state: .working,
                                 name: record.name, stateSince: now, cwd: record.cwd),
                phase: .hidden, pid: record.pid, lastEvent: now)
            entry.session.name = record.name
            entry.session.cwd = record.cwd
            entry.pid = record.pid
            if entries[key] == nil {
                entry.session.home = home(record.pid)
            }
            if let reported {
                transition(&entry, to: reported, now: now)
                entry.session.detail = detail
            } else if entry.phase == .working || entry.phase == .needsYou {
                transition(&entry, to: .done, now: now)
                entry.session.detail = ""
            }
            entries[key] = entry
        }
        entries = entries.filter { !$0.key.hasPrefix("claude:") || seen.contains($0.key) }
    }

    static func waitingDetail(_ waitingFor: String?) -> String {
        guard let waitingFor else { return "?" }
        return waitingFor.localizedCaseInsensitiveContains("permission") ? "perm" : "?"
    }

    // MARK: Codex

    public mutating func apply(_ event: CodexEvent, now: Date, name: String?, home: SessionHome,
                               pid: Int32?) {
        let key = "codex:\(event.sessionId)"
        if event.kind == .sessionEnd {
            entries[key] = nil
            return
        }
        // A subagent's own tool calls don't change its parent's row; only its coming and going.
        if event.agentId != nil && event.kind != .subagentStart && event.kind != .subagentStop {
            return
        }

        var entry = entries[key] ?? Entry(
            session: Session(id: sessionID(key), agent: .codex, state: .working,
                             name: name ?? Self.fallbackName(event.cwd), stateSince: now,
                             cwd: event.cwd, home: home),
            phase: .hidden, pid: pid, lastEvent: now)
        if let name { entry.session.name = name }
        if !event.cwd.isEmpty { entry.session.cwd = event.cwd }
        if home != .unknown { entry.session.home = home }
        if let pid { entry.pid = pid }
        entry.lastEvent = now

        switch event.kind {
        case .sessionStart, .sessionEnd:
            break
        case .userPromptSubmit:
            transition(&entry, to: .working, now: now)
            entry.session.detail = ""
        case .preToolUse:
            transition(&entry, to: .working, now: now)
            entry.session.detail = event.toolName ?? ""
        case .postToolUse:
            transition(&entry, to: .working, now: now)
        case .permissionRequest:
            transition(&entry, to: .needsYou, now: now)
            entry.session.detail = "perm"
        case .stop:
            if entry.phase == .working || entry.phase == .needsYou {
                transition(&entry, to: .done, now: now)
            }
            entry.session.detail = ""
            entry.session.subagents = 0
        case .interrupt:
            transition(&entry, to: .hidden, now: now)  // you're right there
        case .subagentStart:
            entry.session.subagents = entry.session.subagents &+ (entry.session.subagents < 255 ? 1 : 0)
        case .subagentStop:
            entry.session.subagents = entry.session.subagents > 0 ? entry.session.subagents - 1 : 0
        }
        entries[key] = entry
    }

    static func fallbackName(_ cwd: String) -> String {
        cwd.isEmpty ? "Codex" : URL(fileURLWithPath: cwd).lastPathComponent
    }

    // MARK: Housekeeping

    /// Hides a DONE session until it changes state again. Returns false for anything else.
    @discardableResult
    public mutating func dismiss(id: UInt32) -> Bool {
        guard let (key, entry) = entries.first(where: { $0.value.session.id == id }),
              entry.phase == .done
        else { return false }
        entries[key]?.phase = .hidden
        return true
    }

    /// Drops Codex sessions whose process is gone, quiets orphans and sweeps day-old DONE rows.
    public mutating func prune(now: Date, isAlive: (Int32) -> Bool) {
        for (key, entry) in entries {
            if key.hasPrefix("codex:"), let pid = entry.pid, !isAlive(pid) {
                entries[key] = nil
                continue
            }
            if entry.phase == .done && now.timeIntervalSince(entry.session.stateSince) > Self.doneSweep {
                entries[key]?.phase = .hidden
            }
            if key.hasPrefix("codex:"), entry.pid == nil, entry.phase == .working,
               now.timeIntervalSince(entry.lastEvent) > Self.orphanTimeout {
                entries[key]?.phase = .hidden
            }
        }
    }

    // MARK: Reading

    /// What the clock should show, in board order.
    public func visible() -> [Session] {
        entries.values
            .filter { $0.phase != .hidden && filter.admits(cwd: $0.session.cwd) }
            .map(\.session)
            .sorted(by: Session.boardOrder)
    }

    public func session(id: UInt32) -> Session? {
        entries.values.first { $0.session.id == id }?.session
    }

    private func transition(_ entry: inout Entry, to phase: Phase, now: Date) {
        guard entry.phase != phase else { return }
        entry.phase = phase
        entry.session.stateSince = now
        switch phase {
        case .working: entry.session.state = .working
        case .needsYou: entry.session.state = .needsYou
        case .done: entry.session.state = .done
        case .hidden: break
        }
    }
}
