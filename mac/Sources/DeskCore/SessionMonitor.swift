import Foundation
import Observation

/// Watches every source and keeps the current board: Claude's registry (polled every second),
/// Codex hook events (pushed over the hook socket) and periodic pruning.
@MainActor
@Observable
public final class SessionMonitor {
    public private(set) var sessions: [Session] = []
    public private(set) var hookServerError: String?

    public var filter: SessionFilter {
        get { tracker.filter }
        set {
            tracker.filter = newValue
            publish()
        }
    }

    /// Called whenever `sessions` changes.
    @ObservationIgnored public var onChange: (([Session]) -> Void)?

    @ObservationIgnored private var tracker = SessionTracker()
    @ObservationIgnored private let threads = CodexThreads()
    @ObservationIgnored private var hookServer: HookServer?
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var ticks = 0

    public init() {}

    public func start() {
        let server = HookServer(queue: .main) { [weak self] line in
            MainActor.assumeIsolated { self?.received(line) }
        }
        do {
            try server.start()
            hookServer = server
        } catch {
            hookServerError = "Couldn't listen for Codex hooks: \(error.localizedDescription)"
        }
        poll()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.poll() }
        }
    }

    public func stop() {
        timer?.invalidate()
        hookServer?.stop()
    }

    public func dismiss(id: UInt32) {
        if tracker.dismiss(id: id) { publish() }
    }

    public func session(id: UInt32) -> Session? { tracker.session(id: id) }

    private func poll() {
        let now = Date()
        tracker.updateClaude(ClaudeSessions.read(), now: now, isAlive: Processes.isAlive,
                             home: Processes.cmuxHome(of:))
        ticks += 1
        if ticks % 10 == 0 {
            tracker.prune(now: now, isAlive: Processes.isAlive)
        }
        publish()
    }

    private func received(_ line: Data) {
        guard let event = CodexEvent.parse(line: line) else { return }
        let (home, pid) = locate(event)
        tracker.apply(event, now: Date(), name: threads.title(of: event.sessionId), home: home, pid: pid)
        publish()
    }

    /// Where a Codex session runs: a cmux surface, the Codex Desktop app, or somewhere else; and
    /// which process to watch so the row disappears when Codex quits.
    private func locate(_ event: CodexEvent) -> (SessionHome, Int32?) {
        guard let parent = event.parentPid else { return (.unknown, nil) }
        let lineage = Processes.lineage(of: parent)
        let codex = lineage.first { Processes.executablePath(of: $0)?.hasSuffix("/codex") ?? false }
        if let surface = event.cmuxSurface {
            return (.cmux(workspace: event.cmuxWorkspace, surface: surface), codex)
        }
        let desktop = lineage.contains {
            let path = Processes.executablePath(of: $0) ?? ""
            return path.contains("/ChatGPT.app/") || path.contains("/Codex.app/")
        }
        return (desktop ? .codexDesktop : .unknown, codex)
    }

    private func publish() {
        let next = tracker.visible()
        guard next != sessions else { return }
        sessions = next
        onChange?(next)
    }
}
