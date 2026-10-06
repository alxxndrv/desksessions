import Foundation
import Testing

@testable import DeskCore

private func golden() throws -> [UInt8] {
    let url = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("protocol/golden/frame-v1.hex")
    let hex = try String(contentsOf: url, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
    return stride(from: 0, to: hex.count, by: 2).map {
        let start = hex.index(hex.startIndex, offsetBy: $0)
        return UInt8(hex[start..<hex.index(start, offsetBy: 2)], radix: 16)!
    }
}

@Test func encodesTheGoldenFrame() throws {
    let now = Date(timeIntervalSince1970: 1_000_000)
    let frame = Wire.Frame(
        sessions: [
            Session(id: 0x0102_0304, agent: .claude, state: .needsYou, name: "servarium-05",
                    detail: "perm", stateSince: now.addingTimeInterval(-42)),
            Session(id: 0xA0B0_C0D0, agent: .codex, state: .working, name: "Миграция базы",
                    subagents: 3, stateSince: now.addingTimeInterval(-3600)),
        ],
        secondsOfDay: 12 * 3600 + 34 * 60 + 56, nightStart: 23, nightEnd: 8,
        dayBrightness: 80, nightBrightness: 12, flags: [.swapButtons], now: now)
    #expect(Wire.encode(frame) == (try golden()))
}

@Test func cutsNamesOnCharacterBoundaries() {
    let now = Date()
    let name = String(repeating: "a", count: 47) + "ж"
    let frame = Wire.Frame(
        sessions: [Session(id: 1, agent: .claude, state: .done, name: name, stateSince: now)],
        secondsOfDay: 0, nightStart: 23, nightEnd: 8, flags: [], now: now)
    let bytes = Wire.encode(frame)
    #expect(bytes[11 + 11] == 47)  // name length byte after the header and fixed session fields
}

@Test func chunksCarryHeadersAndReassemble() {
    let frame = [UInt8](repeating: 7, count: 1000)
    let chunks = Wire.chunks(frame, sequence: 9, maxWrite: 185)
    #expect(chunks.count == 6)
    #expect(chunks.allSatisfy { $0.count <= 185 && $0[0] == 9 && $0[2] == 6 })
    #expect(chunks.enumerated().allSatisfy { $0.element[1] == UInt8($0.offset) })
    #expect(chunks.flatMap { $0.dropFirst(3) } == frame)
}

@Test func decodesCommands() {
    #expect(Wire.decodeCommand([0, 4, 3, 2, 1]) == .dismiss(0x0102_0304))
    #expect(Wire.decodeCommand([1, 4, 3, 2, 1]) == .jump(0x0102_0304))
    #expect(Wire.decodeCommand([2, 0, 0, 0, 0]) == nil)
    #expect(Wire.decodeCommand([0]) == nil)
}

@Test func sessionIDsAreStable() {
    #expect(sessionID("claude:abc") == sessionID("claude:abc"))
    #expect(sessionID("claude:abc") != sessionID("codex:abc"))
}
