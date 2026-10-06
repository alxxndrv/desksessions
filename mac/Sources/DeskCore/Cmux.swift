import Foundation

/// Drives cmux through its CLI to bring a terminal session to the front.
public enum Cmux {
    public static let cli = "/Applications/cmux.app/Contents/Resources/bin/cmux"

    public static var isInstalled: Bool { FileManager.default.isExecutableFile(atPath: cli) }

    /// Selects the workspace and focuses the surface. `password` is cmux's socket password, for
    /// when its socket only admits processes started inside cmux.
    public static func focus(workspace: String?, surface: String, password: String?) async -> Bool {
        var global: [String] = []
        if let password, !password.isEmpty { global = ["--password", password] }
        if let workspace {
            guard await run(global + ["select-workspace", "--workspace", workspace]) else { return false }
        }
        var focus = global + ["focus-panel", "--panel", surface]
        if let workspace { focus += ["--workspace", workspace] }
        return await run(focus)
    }

    static func run(_ arguments: [String]) async -> Bool {
        await withCheckedContinuation { continuation in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: cli)
            process.arguments = arguments
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            process.terminationHandler = { continuation.resume(returning: $0.terminationStatus == 0) }
            do {
                try process.run()
            } catch {
                continuation.resume(returning: false)
            }
        }
    }
}
