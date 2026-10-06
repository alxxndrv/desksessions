import DeskCore
import SwiftUI

/// One session, styled like the clock's rows: state bar, agent glyph, name, detail, time.
struct SessionRow: View {
    let session: Session
    let selected: Bool
    let selection: Namespace.ID
    let shake: Int

    var body: some View {
        HStack(spacing: 10) {
            StateBar(state: session.state)
            AgentGlyph(session: session)
            VStack(alignment: .leading, spacing: 1) {
                Text(session.name)
                    .font(.system(size: 14, weight: .medium))
                    .lineLimit(1)
                if !session.cwd.isEmpty {
                    Text((session.cwd as NSString).abbreviatingWithTildeInPath)
                        .font(.system(size: 11))
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
            }
            Spacer(minLength: 8)
            if session.subagents > 0 {
                Text("×\(session.subagents)").font(.system(size: 11)).foregroundStyle(.secondary)
            } else if !session.detail.isEmpty {
                Text(session.detail).font(.system(size: 11)).foregroundStyle(.secondary)
            }
            Elapsed(since: session.stateSince, state: session.state)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background {
            if selected {
                RoundedRectangle(cornerRadius: 9)
                    .fill(.white.opacity(0.09))
                    .matchedGeometryEffect(id: "highlight", in: selection)
            }
        }
        .keyframeAnimator(initialValue: 0.0, trigger: shake) { view, x in
            view.offset(x: x)
        } keyframes: { _ in
            SpringKeyframe(7, duration: 0.06)
            SpringKeyframe(-6, duration: 0.08)
            SpringKeyframe(3, duration: 0.08)
            SpringKeyframe(0, spring: .bouncy)
        }
    }
}

private struct StateBar: View {
    let state: SessionState

    var body: some View {
        Capsule()
            .fill(color)
            .frame(width: 4, height: 22)
            .phaseAnimator([false, true], trigger: state == .needsYou) { bar, dim in
                bar.opacity(state == .needsYou && dim ? 0.45 : 1)
            } animation: { _ in state == .needsYou ? .easeInOut(duration: 0.7).delay(0.2) : nil }
            .animation(.snappy, value: state)
    }

    private var color: Color {
        switch state {
        case .needsYou: Color(red: 1, green: 0.27, blue: 0.23)
        case .done: Color(red: 1, green: 0.84, blue: 0.04)
        case .working: Color(red: 0.19, green: 0.82, blue: 0.35)
        }
    }
}

private struct AgentGlyph: View {
    let session: Session

    var body: some View {
        Group {
            switch session.state {
            case .done:
                Image(systemName: "checkmark")
                    .foregroundStyle(Color(red: 1, green: 0.84, blue: 0.04))
                    .transition(.symbolEffect(.drawOn))
            case .working:
                Image(systemName: symbol)
                    .foregroundStyle(tint)
                    .symbolEffect(.rotate, options: .repeating)
            case .needsYou:
                Image(systemName: symbol)
                    .foregroundStyle(tint)
                    .symbolEffect(.bounce.up, options: .repeat(.periodic(delay: 2.6)))
            }
        }
        .font(.system(size: 13, weight: .semibold))
        .frame(width: 18)
    }

    private var symbol: String { session.agent == .claude ? "sparkle" : "hexagon" }
    private var tint: Color {
        session.agent == .claude ? Color(red: 0.85, green: 0.47, blue: 0.34) : .primary.opacity(0.85)
    }
}

private struct Elapsed: View {
    let since: Date
    let state: SessionState

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            Text(Self.format(context.date.timeIntervalSince(since), done: state == .done))
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(.tertiary)
                .contentTransition(.numericText())
        }
        .frame(minWidth: 44, alignment: .trailing)
    }

    static func format(_ seconds: TimeInterval, done: Bool) -> String {
        let total = max(0, Int(seconds))
        if done {
            if total < 60 { return "now" }
            return total < 3600 ? "\(total / 60)m" : "\(total / 3600)h"
        }
        if total < 3600 { return String(format: "%d:%02d", total / 60, total % 60) }
        return String(format: "%dh%02d", total / 3600, (total / 60) % 60)
    }
}
