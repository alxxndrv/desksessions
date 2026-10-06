import Foundation
import Testing

@testable import DeskCore

private func line(_ object: [String: Any]) -> String {
    String(decoding: try! JSONSerialization.data(withJSONObject: object), as: UTF8.self)
}

@Test func latestAITitleWins() {
    let data = Data([
        line(["type": "ai-title", "aiTitle": "First idea"]),
        line(["type": "user", "message": "hi"]),
        line(["type": "ai-title", "aiTitle": "Servarium коммерческие страницы"]),
    ].joined(separator: "\n").utf8)
    #expect(ClaudeTitles.latestTitle(in: data) == .init(text: "Servarium коммерческие страницы", isCustom: false))
}

@Test func renameBeatsAITitles() {
    let data = Data([
        line(["type": "custom-title", "customTitle": "backup-audit"]),
        line(["type": "ai-title", "aiTitle": "Later AI title"]),
    ].joined(separator: "\n").utf8)
    #expect(ClaudeTitles.latestTitle(in: data) == .init(text: "backup-audit", isCustom: true))
}

@Test func findsTheTranscriptAndReadsOnlyItsTail() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let folder = root.appendingPathComponent("-Users-me-code-my-app")
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    let filler = String(repeating: line(["type": "assistant", "text": "x"]) + "\n", count: 2000)
    let body = filler + line(["type": "ai-title", "aiTitle": "Fix the build"]) + "\n"
    try Data(body.utf8).write(to: folder.appendingPathComponent("s1.jsonl"))

    let titles = ClaudeTitles(projects: root, tailBytes: 4096)
    #expect(titles.title(sessionId: "s1", cwd: "/Users/me/code/my_app")?.text == "Fix the build")
    #expect(titles.title(sessionId: "missing", cwd: "/Users/me/code/my_app") == nil)
}
