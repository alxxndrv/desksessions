import Foundation
import Testing

@testable import DeskCore

private func temporaryDirectory() -> URL {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

@Test func installKeepsOtherHooksAndUninstallRestoresThem() throws {
    let dir = temporaryDirectory()
    let file = dir.appendingPathComponent("hooks.json")
    let existing = #"{"hooks":{"Stop":[{"matcher":".*","hooks":[{"type":"command","command":"'/x/xirp/stop.cjs'"}]}]}}"#
    try Data(existing.utf8).write(to: file)
    let installer = HooksInstaller(file: file)

    try installer.install(hookPath: "/Apps/DeskSessions.app/Contents/MacOS/desksessions-hook")
    #expect(installer.status() == .installed(hookPath: "/Apps/DeskSessions.app/Contents/MacOS/desksessions-hook"))
    var root = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    var hooks = try #require(root["hooks"] as? [String: Any])
    #expect((hooks["Stop"] as? [Any])?.count == 2)
    #expect(Set(hooks.keys) == Set(HooksInstaller.events))

    // Reinstalling from a moved app repoints instead of duplicating.
    try installer.install(hookPath: "/Other/desksessions-hook")
    root = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    hooks = try #require(root["hooks"] as? [String: Any])
    #expect((hooks["Stop"] as? [Any])?.count == 2)

    try installer.uninstall()
    #expect(installer.status() == .notInstalled)
    root = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    hooks = try #require(root["hooks"] as? [String: Any])
    #expect(Array(hooks.keys) == ["Stop"])
    let backups = try FileManager.default.contentsOfDirectory(atPath: dir.path).filter { $0.contains(".bak-") }
    #expect(!backups.isEmpty)
}

@Test func envelopeKeepsOnlyWhatWeUse() throws {
    let payload = #"{"hook_event_name":"PreToolUse","session_id":"s","cwd":"/x","tool_name":"shell","tool_input":{"command":"secret stuff"}}"#
    let line = try #require(HookSocket.envelope(
        stdin: Data(payload.utf8), environment: ["CMUX_SURFACE_ID": "S", "HOME": "/Users/me"], parentPid: 7))
    let text = String(decoding: line, as: UTF8.self)
    #expect(!text.contains("secret"))
    #expect(!text.contains("HOME"))
    let event = try #require(CodexEvent.parse(line: line.dropLast()))
    #expect(event.toolName == "shell")
    #expect(event.cmuxSurface == "S")
    #expect(event.parentPid == 7)
}

@Test func hookLinesReachTheServer() async throws {
    let path = FileManager.default.temporaryDirectory.appendingPathComponent("dsk-\(UInt32.random(in: 0...99999)).sock").path
    let received = AsyncStream<Data>.makeStream()
    let server = HookServer(path: path, queue: DispatchQueue(label: "test")) { received.continuation.yield($0) }
    try server.start()
    defer { server.stop() }
    #expect(HookSocket.send(Data("{\"hello\":1}\n".utf8), to: path))
    var iterator = received.stream.makeAsyncIterator()
    let line = await iterator.next()
    #expect(line.map { String(decoding: $0, as: UTF8.self) } == "{\"hello\":1}")
}

@Test func sendingWithNoAppRunningFailsQuietly() {
    #expect(!HookSocket.send(Data("x\n".utf8), to: "/tmp/definitely-not-a-socket-\(UUID().uuidString)"))
}
