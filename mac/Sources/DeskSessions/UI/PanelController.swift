import AppKit
import SwiftUI

/// A borderless, non-activating floating panel that can still take typing, like Raycast's.
final class KeyablePanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class PanelController: NSObject, NSWindowDelegate {
    private let panel: KeyablePanel
    private let model: PanelModel

    init(model: PanelModel) {
        self.model = model
        panel = KeyablePanel(contentRect: NSRect(x: 0, y: 0, width: 560, height: 420),
                             styleMask: [.borderless, .nonactivatingPanel, .fullSizeContentView],
                             backing: .buffered, defer: true)
        super.init()
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = false
        panel.delegate = self
        let host = NSHostingView(rootView: PanelView(model: model))
        host.sizingOptions = [.preferredContentSize]
        panel.contentView = host
        model.onClose = { [weak self] in self?.hide() }
    }

    var isVisible: Bool { panel.isVisible }

    /// Called after the panel closes for any reason.
    var onHide: (() -> Void)?

    func toggle(anchor: NSStatusBarButton?) {
        isVisible ? hide() : show(anchor: anchor)
    }

    /// Drops from the menu-bar icon when there is one to anchor to, otherwise centres on screen.
    func show(anchor: NSStatusBarButton?) {
        model.reset()
        model.presented = false
        let measured = panel.contentView?.fittingSize ?? .zero
        let size = measured.width > 10 && measured.height > 10 ? measured : NSSize(width: 560, height: 420)
        let screen = anchor?.window?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        let visible = screen.visibleFrame
        var origin: NSPoint
        if let anchor, let window = anchor.window {
            let frame = window.convertToScreen(anchor.convert(anchor.bounds, to: nil))
            origin = NSPoint(x: frame.midX - size.width / 2, y: frame.minY - size.height - 6)
        } else {
            origin = NSPoint(x: visible.midX - size.width / 2, y: visible.midY - size.height / 2 + visible.height * 0.12)
        }
        origin.x = min(max(origin.x, visible.minX + 8), visible.maxX - size.width - 8)
        panel.setFrame(NSRect(origin: origin, size: size), display: false)
        panel.alphaValue = 1
        panel.makeKeyAndOrderFront(nil)
        // The content springs in (SwiftUI .snappy) on the next pass.
        DispatchQueue.main.async { [model] in model.presented = true }
    }

    func hide() {
        guard panel.isVisible, model.presented else { return }
        model.presented = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.16) { [weak self] in
            guard let self, !self.model.presented else { return }
            self.panel.orderOut(nil)
            self.onHide?()
        }
    }

    func windowDidResignKey(_ notification: Notification) {
        hide()
    }
}
