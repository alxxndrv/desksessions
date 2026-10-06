import Foundation
import Testing

@testable import DeskCore

private let t0 = Date(timeIntervalSince1970: 1_000_000)
private let alive: @Sendable (Int32) -> Bool = { _ in true }
private let noHome: @Sendable (Int32) -> SessionHome = { _ in .unknown }

private func claude(_ status: String, pid: Int32 = 1, id: String = "a", cwd: String = "/Users/me/code/app",
                    waitingFor: String? = nil, kind: String = "interactive") -> ClaudeRecord {
    ClaudeRecord(pid: pid, sessionId: id, cwd: cwd, name: "app-\(id)", status: status,
                 waitingFor: waitingFor, kind: kind)
}

@Test func claudeBusyThenIdleBecomesDone() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("busy")], now: t0, isAlive: alive, home: noHome)
    #expect(tracker.visible().map(\.state) == [.working])
    tracker.updateClaude([claude("idle")], now: t0 + 5, isAlive: alive, home: noHome)
    #expect(tracker.visible().map(\.state) == [.done])
    #expect(tracker.visible().first?.stateSince == t0 + 5)
}

@Test func claudeAlreadyIdleAtLaunchStaysHidden() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("idle")], now: t0, isAlive: alive, home: noHome)
    #expect(tracker.visible().isEmpty)
}

@Test func claudeWaitingNeedsYouWithReason() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("waiting", waitingFor: "permission prompt")], now: t0, isAlive: alive, home: noHome)
    #expect(tracker.visible().first?.state == .needsYou)
    #expect(tracker.visible().first?.detail == "perm")
    tracker.updateClaude([claude("waiting", waitingFor: "dialog open")], now: t0, isAlive: alive, home: noHome)
    #expect(tracker.visible().first?.detail == "?")
}

@Test func claudeFiltersHeadlessTempAndDeadSessions() {
    var tracker = SessionTracker()
    tracker.updateClaude([
        claude("busy", pid: 1, id: "a"),
        claude("busy", pid: 2, id: "b", kind: "print"),
        claude("busy", pid: 3, id: "c", cwd: "/private/var/folders/x/T/title"),
        claude("busy", pid: 4, id: "d"),
    ], now: t0, isAlive: { $0 != 4 }, home: noHome)
    #expect(tracker.visible().map(\.name) == ["app-a"])
}

@Test func claudeSessionsDisappearWhenTheirFileGoes() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("busy")], now: t0, isAlive: alive, home: noHome)
    tracker.updateClaude([], now: t0 + 1, isAlive: alive, home: noHome)
    #expect(tracker.visible().isEmpty)
}

@Test func dismissHidesDoneUntilWorkResumes() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("busy")], now: t0, isAlive: alive, home: noHome)
    tracker.updateClaude([claude("idle")], now: t0 + 1, isAlive: alive, home: noHome)
    let id = tracker.visible()[0].id
    let dismissed = tracker.dismiss(id: id)
    #expect(dismissed)
    tracker.updateClaude([claude("idle")], now: t0 + 2, isAlive: alive, home: noHome)
    #expect(tracker.visible().isEmpty)
    tracker.updateClaude([claude("busy")], now: t0 + 3, isAlive: alive, home: noHome)
    #expect(tracker.visible().map(\.state) == [.working])
}

@Test func onlyDoneCanBeDismissed() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("busy")], now: t0, isAlive: alive, home: noHome)
    let dismissed = tracker.dismiss(id: tracker.visible()[0].id)
    #expect(!dismissed)
}

@Test func codexLifecycle() {
    var tracker = SessionTracker()
    func send(_ kind: CodexEvent.Kind, tool: String? = nil, at offset: TimeInterval) {
        tracker.apply(CodexEvent(kind: kind, sessionId: "t1", cwd: "/Users/me/x", toolName: tool),
                      now: t0 + offset, name: "Fix the thing", home: .codexDesktop, pid: nil)
    }
    send(.sessionStart, at: 0)
    #expect(tracker.visible().isEmpty)
    send(.userPromptSubmit, at: 1)
    send(.preToolUse, tool: "shell", at: 2)
    #expect(tracker.visible().first?.detail == "shell")
    #expect(tracker.visible().first?.stateSince == t0 + 1)  // still the same working stretch
    send(.permissionRequest, at: 3)
    #expect(tracker.visible().first?.state == .needsYou)
    send(.postToolUse, at: 4)
    send(.stop, at: 5)
    #expect(tracker.visible().first?.state == .done)
    #expect(tracker.visible().first?.name == "Fix the thing")
    send(.sessionEnd, at: 6)
    #expect(tracker.visible().isEmpty)
}

@Test func codexSubagentsFoldIntoTheParent() {
    var tracker = SessionTracker()
    let parent = CodexEvent(kind: .userPromptSubmit, sessionId: "p")
    tracker.apply(parent, now: t0, name: nil, home: .unknown, pid: nil)
    for _ in 0..<3 {
        tracker.apply(CodexEvent(kind: .subagentStart, sessionId: "p", agentId: "s"), now: t0, name: nil,
                      home: .unknown, pid: nil)
    }
    tracker.apply(CodexEvent(kind: .preToolUse, sessionId: "p", toolName: "x", agentId: "s"), now: t0,
                  name: nil, home: .unknown, pid: nil)
    tracker.apply(CodexEvent(kind: .subagentStop, sessionId: "p", agentId: "s"), now: t0, name: nil,
                  home: .unknown, pid: nil)
    let row = tracker.visible()[0]
    #expect(row.subagents == 2)
    #expect(row.detail == "")
}

@Test func pruneDropsDeadCodexAndSweepsOldDone() {
    var tracker = SessionTracker()
    tracker.apply(CodexEvent(kind: .userPromptSubmit, sessionId: "dead"), now: t0, name: nil,
                  home: .unknown, pid: 99)
    tracker.updateClaude([claude("busy")], now: t0, isAlive: alive, home: noHome)
    tracker.updateClaude([claude("idle")], now: t0 + 1, isAlive: alive, home: noHome)
    tracker.prune(now: t0 + 2, isAlive: { $0 != 99 })
    #expect(tracker.visible().map(\.agent) == [.claude])
    tracker.prune(now: t0 + SessionTracker.doneSweep + 10, isAlive: alive)
    #expect(tracker.visible().isEmpty)
}

@Test func filterPatternsApply() {
    var tracker = SessionTracker(filter: SessionFilter(deny: ["*/scratch*"]))
    tracker.updateClaude([claude("busy", cwd: "/Users/me/scratch-pad")], now: t0, isAlive: alive, home: noHome)
    #expect(tracker.visible().isEmpty)
}

@Test func parsesHookLines() throws {
    let line = #"{"payload":{"hook_event_name":"PreToolUse","session_id":"abc","cwd":"/x","tool_name":"shell"},"env":{"CMUX_SURFACE_ID":"S","CMUX_WORKSPACE_ID":"W"},"ppid":42}"#
    let event = try #require(CodexEvent.parse(line: Data(line.utf8)))
    #expect(event.kind == .preToolUse)
    #expect(event.toolName == "shell")
    #expect(event.parentPid == 42)
    #expect(event.cmuxSurface == "S")
}

@Test func parsesClaudeRegistry() throws {
    let json = #"{"pid":16936,"sessionId":"s1","cwd":"/Users/me/code/servarium","kind":"interactive","name":"servarium-05","status":"waiting","waitingFor":"dialog open"}"#
    let record = try #require(ClaudeSessions.parse(Data(json.utf8)))
    #expect(record.pid == 16936)
    #expect(record.status == "waiting")
    #expect(record.waitingFor == "dialog open")
}

@Test func claudeShellIsABackgroundCommandNotWork() {
    var tracker = SessionTracker()
    tracker.updateClaude([claude("shell")], now: t0, isAlive: alive, home: noHome)
    #expect(tracker.visible().isEmpty)
    tracker.updateClaude([claude("busy")], now: t0 + 1, isAlive: alive, home: noHome)
    tracker.updateClaude([claude("shell")], now: t0 + 2, isAlive: alive, home: noHome)
    #expect(tracker.visible().map(\.state) == [.done])
}
