import Foundation

/// Which sessions belong on the clock: interactive ones, not throwaway runs in temp folders, and
/// whatever the user's allow/deny patterns say.
public struct SessionFilter: Sendable, Equatable {
    public var allow: [String]
    public var deny: [String]

    public static let hiddenPrefixes = ["/var/folders/", "/private/var/folders/", "/tmp/", "/private/tmp/"]

    public init(allow: [String] = [], deny: [String] = []) {
        self.allow = allow
        self.deny = deny
    }

    /// `true` if a session working in `cwd` should be shown. Patterns are globs (`*`, `?`) on
    /// the full path; an allow list, when present, must match.
    public func admits(cwd: String) -> Bool {
        if Self.hiddenPrefixes.contains(where: { cwd.hasPrefix($0) }) { return false }
        if deny.contains(where: { glob($0, cwd) }) { return false }
        return allow.isEmpty || allow.contains(where: { glob($0, cwd) })
    }

    private func glob(_ pattern: String, _ path: String) -> Bool {
        fnmatch(pattern, path, 0) == 0
    }
}
