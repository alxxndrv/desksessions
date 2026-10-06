import AppKit
import DeskCore

/// The menu-bar item: a little clock glyph with a link dot, and the NEEDS YOU count beside it.
@MainActor
final class StatusItemController {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

    init(action: @escaping () -> Void) {
        let target = ClickTarget(action: action)
        self.target = target
        item.button?.target = target
        item.button?.action = #selector(ClickTarget.clicked)
        item.button?.imagePosition = .imageLeading
    }

    private let target: ClickTarget

    var button: NSStatusBarButton? { item.button }

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

    /// A clock face outline (template-tinted by hand) with a coloured dot for the link state.
    private static func icon(link: ClockLink.Status) -> NSImage {
        let dot: NSColor
        switch link {
        case .connected: dot = .systemGreen
        case .searching, .pairing: dot = .systemOrange
        case .notPaired, .bluetoothOff, .unauthorized: dot = .systemGray
        }
        let image = NSImage(size: NSSize(width: 20, height: 14), flipped: false) { rect in
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
            _ = rect
            return true
        }
        image.isTemplate = false
        return image
    }

    private final class ClickTarget: NSObject {
        let action: () -> Void
        init(action: @escaping () -> Void) { self.action = action }
        @objc func clicked() { action() }
    }
}
