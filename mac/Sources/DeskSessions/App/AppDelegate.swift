import AppKit
import DeskCore
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let settings = AppSettings()
    private let monitor = SessionMonitor()
    private let link = ClockLink()
    private var statusItem: StatusItemController!
    private var panel: PanelController!
    private var panelModel: PanelModel!
    private var hotKey: HotKey!
    private var settingsWindow: NSWindow?

    /// The hook binary that ships inside the app bundle, next to the main executable.
    private var hookPath: String {
        Bundle.main.executableURL!.deletingLastPathComponent().appendingPathComponent("desksessions-hook").path
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = StatusItemController(
            onOpen: { [weak self] anchor in self?.panel.show(anchor: anchor) },
            onClose: { [weak self] in self?.panel.hide() })
        panelModel = PanelModel(monitor: monitor, link: link)
        panelModel.onJump = { [weak self] session in self?.jump(to: session) }
        panelModel.onSettings = { [weak self] in self?.openSettings() }
        panel = PanelController(model: panelModel)
        panel.onHide = { [weak self] in self?.statusItem.endSession() }

        hotKey = HotKey { [weak self] in self?.panel.toggle(anchor: nil) }
        hotKey.register(settings.hotKey)

        monitor.filter = settings.filter
        monitor.onChange = { [weak self] _ in
            self?.link.requestSend()
            self?.refreshStatusItem()
        }
        monitor.start()

        link.makeFrame = { [weak self] in self?.frame() ?? Wire.Frame(
            sessions: [], secondsOfDay: 0, nightStart: 23, nightEnd: 8, flags: [], now: Date()) }
        link.onCommand = { [weak self] command in self?.handle(command) }
        link.start()

        repointHooksIfMoved()
        observeSettings()
        observeLink()
        refreshStatusItem()
    }

    func applicationWillTerminate(_ notification: Notification) {
        monitor.stop()
    }

    private func frame() -> Wire.Frame {
        let now = Date()
        let parts = Calendar.current.dateComponents([.hour, .minute, .second], from: now)
        let seconds = (parts.hour ?? 0) * 3600 + (parts.minute ?? 0) * 60 + (parts.second ?? 0)
        return Wire.Frame(sessions: monitor.sessions, secondsOfDay: UInt32(seconds),
                          nightStart: UInt8(settings.nightStart), nightEnd: UInt8(settings.nightEnd),
                          dayBrightness: UInt8(clamping: settings.dayBrightness),
                          nightBrightness: UInt8(clamping: settings.nightBrightness),
                          flags: settings.flags, now: now)
    }

    private func handle(_ command: Wire.Command) {
        switch command {
        case let .dismiss(id):
            monitor.dismiss(id: id)
        case let .jump(id):
            if let session = monitor.session(id: id) { jump(to: session) }
        }
    }

    private func jump(to session: Session) {
        Jump.to(session, cmuxPassword: settings.cmuxPassword)
    }

    private func refreshStatusItem() {
        statusItem.update(needsYou: monitor.sessions.filter { $0.state == .needsYou }.count, link: link.status)
    }

    /// If the app moved since the hooks were installed, point them at the new location.
    private func repointHooksIfMoved() {
        let installer = HooksInstaller()
        if case let .installed(path) = installer.status(), path != hookPath {
            try? installer.install(hookPath: hookPath)
        }
    }

    private func observeSettings() {
        withObservationTracking {
            _ = settings.flags
            _ = settings.nightStart
            _ = settings.nightEnd
            _ = settings.dayBrightness
            _ = settings.nightBrightness
            _ = settings.filter
        } onChange: { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.monitor.filter = self.settings.filter
                self.link.requestSend()
                self.observeSettings()
            }
        }
    }

    private func observeLink() {
        withObservationTracking {
            _ = link.status
        } onChange: { [weak self] in
            Task { @MainActor in
                self?.refreshStatusItem()
                self?.observeLink()
            }
        }
    }

    private func openSettings() {
        panel.hide()
        if settingsWindow == nil {
            let view = SettingsView(settings: settings, link: link, monitor: monitor, hookPath: hookPath) {
                [weak self] combo in self?.hotKey.register(combo)
            }
            let window = NSWindow(contentViewController: NSHostingController(rootView: view))
            window.title = "DeskSessions"
            window.styleMask = [.titled, .closable]
            window.isReleasedWhenClosed = false
            settingsWindow = window
        }
        settingsWindow?.center()
        NSApp.activate()
        settingsWindow?.makeKeyAndOrderFront(nil)
    }
}
