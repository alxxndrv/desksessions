import AppKit
import DeskCore

/// The menu-bar item: a little clock glyph with a link dot, and the NEEDS YOU count beside it.
///
/// On macOS 27 a status item that opens its own window must do so through an "expanded
/// interface session": the menu bar tells us when to open and close, which also gives the item
/// its pressed look and keyboard navigation. Earlier systems use plain target/action.
@MainActor
final class StatusItemController: NSObject {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let onOpen: (NSStatusBarButton?) -> Void
    private let onClose: () -> Void

    init(onOpen: @escaping (NSStatusBarButton?) -> Void, onClose: @escaping () -> Void) {
        self.onOpen = onOpen
        self.onClose = onClose
        super.init()
        item.button?.imagePosition = .imageLeading
        if #available(macOS 27, *) {
            item.expandedInterfaceDelegate = self
        } else {
            item.button?.target = self
            item.button?.action = #selector(clicked)
        }
    }

    var button: NSStatusBarButton? { item.button }

    /// Our panel closed by itself (Esc, a click elsewhere, a jump): tell the menu bar.
    func endSession() {
        if #available(macOS 27, *) {
            item.expandedInterfaceSession?.cancel()
        }
    }

    @objc private func clicked() {
        onOpen(item.button)
    }

    func update(needsYou: Int, link: ClockLink.Status) {
        item.button?.image = Self.icon(link: link)
        if needsYou > 0 {
            item.button?.attributedTitle = NSAttributedString(
                string: " \(needsYou)",
                attributes: [.foregroundColor: NSColor.systemRed,
                             .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .semibold)])
        } else {
            item.button?.title = ""
        }
    }

    /// A clock face outline with a coloured dot for the link state.
    private static func icon(link: ClockLink.Status) -> NSImage {
        let dot: NSColor
        switch link {
        case .connected: dot = .systemGreen
        case .searching, .pairing: dot = .systemOrange
        case .notPaired, .bluetoothOff, .unauthorized: dot = .systemGray
        }
        let image = NSImage(size: NSSize(width: 20, height: 14), flipped: false) { _ in
            let body = NSBezierPath(roundedRect: NSRect(x: 1, y: 2, width: 15, height: 10), xRadius: 2.5, yRadius: 2.5)
            body.lineWidth = 1.4
            NSColor.labelColor.setStroke()
            body.stroke()
            for row in 0..<2 {
                let line = NSBezierPath()
                line.move(to: NSPoint(x: 4, y: 8.5 - CGFloat(row) * 3))
                line.line(to: NSPoint(x: 12, y: 8.5 - CGFloat(row) * 3))
                line.lineWidth = 1.2
                NSColor.labelColor.withAlphaComponent(0.7).setStroke()
                line.stroke()
            }
            dot.setFill()
            NSBezierPath(ovalIn: NSRect(x: 14, y: 0, width: 6, height: 6)).fill()
            return true
        }
        image.isTemplate = false
        return image
    }
}

@available(macOS 27, *)
extension StatusItemController: @MainActor NSStatusItemExpandedInterfaceDelegate {
    func statusItem(_ statusItem: NSStatusItem,
                    didBegin session: NSStatusItemExpandedInterfaceSession) {
        onOpen(statusItem.button)
    }

    func statusItemDidEndExpandedInterfaceSession(_ statusItem: NSStatusItem, animated: Bool) {
        onClose()
    }
}
