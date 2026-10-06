import Foundation

/// The BLE wire format; see protocol/PROTOCOL.md. Mirrors firmware/lib/core/src/protocol/wire.cpp.
public enum Wire {
    public static let version: UInt8 = 1
    public static let chunkHeaderBytes = 3
    public static let maxChunks = 8
    public static let maxSessions = 16
    public static let maxNameBytes = 48
    public static let maxDetailBytes = 12

    public static let serviceUUID = "b4d4f873-8133-48f0-9479-88481c6d52cf"
    public static let snapshotUUID = "8c9ccb7b-95a9-411e-a3db-00b974bbb1ea"
    public static let eventsUUID = "6fbc6994-1373-4ace-9710-e9bf939d9251"

    public struct Flags: OptionSet, Sendable {
        public let rawValue: UInt8
        public init(rawValue: UInt8) { self.rawValue = rawValue }
        public static let flipScreen = Flags(rawValue: 1 << 0)
        public static let swapButtons = Flags(rawValue: 1 << 1)
    }

    public struct Frame: Sendable {
        public var sessions: [Session]
        public var secondsOfDay: UInt32
        public var nightStart: UInt8
        public var nightEnd: UInt8
        public var dayBrightness: UInt8
        public var nightBrightness: UInt8
        public var flags: Flags
        public var now: Date

        public init(sessions: [Session], secondsOfDay: UInt32, nightStart: UInt8, nightEnd: UInt8,
                    dayBrightness: UInt8 = 100, nightBrightness: UInt8 = 10, flags: Flags, now: Date) {
            self.sessions = sessions
            self.secondsOfDay = secondsOfDay
            self.nightStart = nightStart
            self.nightEnd = nightEnd
            self.dayBrightness = min(dayBrightness, 100)
            self.nightBrightness = min(nightBrightness, 100)
            self.flags = flags
            self.now = now
        }
    }

    public enum Command: Equatable, Sendable {
        case dismiss(UInt32)
        case jump(UInt32)
    }

    /// Serialises a frame. Sessions beyond 16 are dropped; strings are cut on UTF-8 boundaries.
    public static func encode(_ frame: Frame) -> [UInt8] {
        var out: [UInt8] = [version, frame.flags.rawValue]
        out.appendLE(frame.secondsOfDay)
        out.append(frame.nightStart)
        out.append(frame.nightEnd)
        out.append(frame.dayBrightness)
        out.append(frame.nightBrightness)
        let sessions = frame.sessions.prefix(maxSessions)
        out.append(UInt8(sessions.count))
        for s in sessions {
            out.appendLE(s.id)
            out.append(s.agent.rawValue)
            out.append(s.state.rawValue)
            out.append(s.subagents)
            let seconds = max(0, frame.now.timeIntervalSince(s.stateSince))
            out.appendLE(UInt32(min(seconds, Double(UInt32.max))))
            out.appendString(s.name, limit: maxNameBytes)
            out.appendString(s.detail, limit: maxDetailBytes)
        }
        return out
    }

    /// Splits an encoded frame into ATT-sized chunks, each prefixed with (frame, index, count).
    public static func chunks(_ frame: [UInt8], sequence: UInt8, maxWrite: Int) -> [[UInt8]] {
        let payload = max(1, min(maxWrite, 512) - chunkHeaderBytes)
        let count = max(1, (frame.count + payload - 1) / payload)
        return (0..<count).map { index in
            let start = index * payload
            let slice = frame[start..<min(frame.count, start + payload)]
            return [sequence, UInt8(index), UInt8(count)] + slice
        }
    }

    public static func decodeCommand(_ bytes: [UInt8]) -> Command? {
        guard bytes.count == 5 else { return nil }
        let id = UInt32(bytes[1]) | UInt32(bytes[2]) << 8 | UInt32(bytes[3]) << 16 | UInt32(bytes[4]) << 24
        switch bytes[0] {
        case 0: return .dismiss(id)
        case 1: return .jump(id)
        default: return nil
        }
    }
}

extension Array where Element == UInt8 {
    mutating func appendLE(_ value: UInt32) {
        append(UInt8(truncatingIfNeeded: value))
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value >> 16))
        append(UInt8(truncatingIfNeeded: value >> 24))
    }

    mutating func appendString(_ text: String, limit: Int) {
        let bytes = Array(text.utf8)
        var n = Swift.min(bytes.count, limit)
        if n < bytes.count {
            while n > 0 && (bytes[n] & 0xC0) == 0x80 { n -= 1 }  // don't split a character
        }
        append(UInt8(n))
        append(contentsOf: bytes[0..<n])
    }
}
