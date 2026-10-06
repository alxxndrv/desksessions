import AppKit
import DeskCore

/// Brings a session's window to the front.
@MainActor
enum Jump {
    static func to(_ session: Session, cmuxPassword: String) {
        switch session.home {
        case let .cmux(workspace, surface):
            Task {
                _ = await Cmux.focus(workspace: workspace, surface: surface, password: cmuxPassword)
                activate(bundleID: "com.cmuxterm.app")
            }
        case .codexDesktop:
            activate(bundleID: "com.openai.codex")
        case .unknown:
            activate(bundleID: session.agent == .codex ? "com.openai.codex" : "com.cmuxterm.app")
        }
    }

    private static func activate(bundleID: String) {
        NSRunningApplication.runningApplications(withBundleIdentifier: bundleID).first?.activate()
    }
}
