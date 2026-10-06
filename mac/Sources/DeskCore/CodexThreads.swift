import Foundation
import SQLite3

/// Reads thread titles from Codex's own state database, read-only.
public final class CodexThreads: @unchecked Sendable {
    private let path: String
    private var cache: [String: String] = [:]
    private let lock = NSLock()

    public init(path: String = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".codex/state_5.sqlite").path) {
        self.path = path
    }

    /// The thread's name as Codex shows it, if it has one yet.
    public func title(of threadId: String) -> String? {
        lock.lock()
        defer { lock.unlock() }
        if let cached = cache[threadId] { return cached }
        guard let title = query(threadId) else { return nil }
        cache[threadId] = title
        return title
    }

    private func query(_ threadId: String) -> String? {
        var db: OpaquePointer?
        guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else {
            sqlite3_close(db)
            return nil
        }
        defer { sqlite3_close(db) }
        sqlite3_busy_timeout(db, 200)
        var statement: OpaquePointer?
        let sql = "SELECT COALESCE(NULLIF(name, ''), NULLIF(title, ''), NULLIF(preview, '')) FROM threads WHERE id = ?"
        guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else { return nil }
        defer { sqlite3_finalize(statement) }
        sqlite3_bind_text(statement, 1, threadId, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self))
        guard sqlite3_step(statement) == SQLITE_ROW, let text = sqlite3_column_text(statement, 0) else {
            return nil
        }
        let title = String(cString: text).trimmingCharacters(in: .whitespacesAndNewlines)
        return title.isEmpty ? nil : String(title.prefix(80))
    }
}
