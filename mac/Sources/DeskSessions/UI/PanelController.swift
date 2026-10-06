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

    func toggle(anchor: NSStatusBarButton?) {
        isVisible ? hide() : show(anchor: anchor)
    }

    /// Drops from the menu-bar icon when there is one to anchor to, otherwise centres on screen.
    func show(anchor: NSStatusBarButton?) {
        model.reset()
        panel.layoutIfNeeded()
        let size = panel.contentView?.fittingSize ?? NSSize(width: 560, height: 420)
        let screen = anchor?.window?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        var origin: NSPoint
        if let anchor, let window = anchor.window {
            let frame = window.convertToScreen(anchor.convert(anchor.bounds, to: nil))
            origin = NSPoint(x: frame.midX - size.width / 2, y: frame.minY - size.height - 6)
        } else {
            let visible = screen.visibleFrame
            origin = NSPoint(x: visible.midX - size.width / 2, y: visible.midY + visible.height * 0.12)
        }
        origin.x = min(max(origin.x, screen.visibleFrame.minX + 8), screen.visibleFrame.maxX - size.width - 8)
        panel.setFrame(NSRect(origin: origin, size: size), display: true)

        // Spring in: a touch small and transparent, settling to full size.
        panel.alphaValue = 0
        panel.makeKeyAndOrderFront(nil)
        if let layer = panel.contentView?.layer {
            layer.anchorPoint = CGPoint(x: 0.5, y: 1)
            layer.position = CGPoint(x: size.width / 2, y: size.height)
            let spring = CASpringAnimation(keyPath: "transform.scale")
            spring.fromValue = 0.94
            spring.toValue = 1
            spring.mass = 1
            spring.stiffness = 158  // SwiftUI .snappy: duration 0.5, bounce 0.15
            spring.damping = 21.4
            spring.duration = spring.settlingDuration
            layer.add(spring, forKey: "drop")
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.16
            panel.animator().alphaValue = 1
        }
    }

    func hide() {
        guard panel.isVisible else { return }
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.12
            panel.animator().alphaValue = 0
        }, completionHandler: { [panel] in
            panel.orderOut(nil)
        })
    }

    func windowDidResignKey(_ notification: Notification) {
        hide()
    }
}
