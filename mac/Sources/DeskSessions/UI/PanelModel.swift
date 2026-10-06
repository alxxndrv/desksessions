import DeskCore
import Foundation
import Observation

/// State behind the floating panel: the filter text, the selection and what each key does.
@MainActor
@Observable
final class PanelModel {
    var query = "" {
        didSet { keepSelectionValid() }
    }
    var selectedID: UInt32?
    /// Drives the panel's spring in and out.
    var presented = false

    let monitor: SessionMonitor
    let link: ClockLink
    @ObservationIgnored var onJump: ((Session) -> Void)?
    @ObservationIgnored var onClose: (() -> Void)?
    @ObservationIgnored var onSettings: (() -> Void)?

    init(monitor: SessionMonitor, link: ClockLink) {
        self.monitor = monitor
        self.link = link
    }

    var rows: [Session] {
        let all = monitor.sessions
        guard !query.isEmpty else { return all }
        return all.filter {
            $0.name.localizedCaseInsensitiveContains(query) || $0.cwd.localizedCaseInsensitiveContains(query)
        }
    }

    func reset() {
        query = ""
        selectedID = rows.first?.id
    }

    func move(_ delta: Int) {
        let rows = rows
        guard !rows.isEmpty else { return }
        let index = rows.firstIndex { $0.id == selectedID } ?? 0
        selectedID = rows[(index + delta + rows.count) % rows.count].id
    }

    func jump() {
        guard let session = rows.first(where: { $0.id == selectedID }) ?? rows.first else { return }
        onJump?(session)
        onClose?()
    }

    /// ⌘⌫ on a DONE row hides it, exactly like the clock's long press.
    func dismissSelected() -> Bool {
        guard let session = rows.first(where: { $0.id == selectedID }), session.state == .done else {
            return false
        }
        move(1)
        monitor.dismiss(id: session.id)
        keepSelectionValid()
        return true
    }

    func keepSelectionValid() {
        if !rows.contains(where: { $0.id == selectedID }) {
            selectedID = rows.first?.id
        }
    }
}
