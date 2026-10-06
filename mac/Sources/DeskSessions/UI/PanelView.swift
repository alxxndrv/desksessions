import DeskCore
import SwiftUI

/// The Raycast-style panel: search field, live session list, action hints.
struct PanelView: View {
    @Bindable var model: PanelModel
    @FocusState private var searchFocused: Bool
    @Namespace private var selection
    @State private var shake = 0

    var body: some View {
        VStack(spacing: 0) {
            searchField
            Divider().opacity(0.4)
            list
            Divider().opacity(0.4)
            footer
        }
        .frame(width: 560)
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
        .onAppear { searchFocused = true }
    }

    private var searchField: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Filter sessions", text: $model.query)
                .textFieldStyle(.plain)
                .font(.system(size: 17))
                .focused($searchFocused)
                .onKeyPress(.upArrow) { model.move(-1); return .handled }
                .onKeyPress(.downArrow) { model.move(1); return .handled }
                .onKeyPress(.return) { model.jump(); return .handled }
                .onKeyPress(.escape) { model.onClose?(); return .handled }
                .onKeyPress(keys: [.delete], phases: .down) { press in
                    guard press.modifiers.contains(.command) else { return .ignored }
                    if !model.dismissSelected() {
                        withAnimation(.snappy) { shake += 1 }
                    }
                    return .handled
                }
                .onKeyPress(keys: [","], phases: .down) { press in
                    guard press.modifiers.contains(.command) else { return .ignored }
                    model.onSettings?()
                    return .handled
                }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    @ViewBuilder private var list: some View {
        let rows = model.rows
        if rows.isEmpty {
            VStack(spacing: 8) {
                Image(systemName: model.query.isEmpty ? "moon.zzz" : "magnifyingglass")
                    .font(.system(size: 28, weight: .light))
                    .foregroundStyle(.secondary)
                    .symbolEffect(.breathe, options: .repeating)
                Text(model.query.isEmpty ? "All quiet" : "No matches")
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, minHeight: 140)
            .transition(.opacity.combined(with: .scale(scale: 0.96)))
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 2) {
                        ForEach(rows) { session in
                            SessionRow(session: session, selected: session.id == model.selectedID,
                                       selection: selection, shake: session.id == model.selectedID ? shake : 0)
                                .id(session.id)
                                .contentShape(Rectangle())
                                .onTapGesture {
                                    model.selectedID = session.id
                                    model.jump()
                                }
                                .onHover { inside in
                                    if inside { model.selectedID = session.id }
                                }
                                .transition(.asymmetric(
                                    insertion: .offset(x: 24).combined(with: .opacity),
                                    removal: .offset(x: -40).combined(with: .opacity)))
                        }
                    }
                    .padding(6)
                    .animation(.snappy, value: rows.map(\.id))
                    .animation(.snappy, value: rows.map(\.state))
                    .animation(.smooth(duration: 0.23), value: model.selectedID)
                }
                .frame(maxHeight: 360)
                .scrollIndicators(.never)
                .onChange(of: model.selectedID) { _, id in
                    withAnimation(.smooth(duration: 0.25)) { proxy.scrollTo(id) }
                }
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 14) {
            LinkBadge(status: model.link.status)
            Spacer()
            Hint(keys: "↵", label: "Jump")
            Hint(keys: "⌘⌫", label: "Dismiss")
            Hint(keys: "⌘,", label: "Settings")
        }
        .font(.system(size: 11))
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
    }
}

private struct Hint: View {
    let keys: String
    let label: String

    var body: some View {
        HStack(spacing: 5) {
            Text(keys)
                .font(.system(size: 10, weight: .semibold, design: .rounded))
                .padding(.horizontal, 5)
                .padding(.vertical, 2)
                .background(.quaternary, in: .rect(cornerRadius: 4))
            Text(label).foregroundStyle(.secondary)
        }
    }
}

struct LinkBadge: View {
    let status: ClockLink.Status

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(color)
                .frame(width: 7, height: 7)
                .phaseAnimator([false, true], trigger: isSearching) { dot, pulse in
                    dot.opacity(isSearching && pulse ? 0.35 : 1)
                } animation: { _ in isSearching ? .easeInOut(duration: 0.9) : nil }
            Text(label).foregroundStyle(.secondary)
                .contentTransition(.opacity)
        }
        .animation(.snappy, value: status)
    }

    private var isSearching: Bool { status == .searching || status == .pairing }

    private var color: Color {
        switch status {
        case .connected: .green
        case .searching, .pairing: .orange
        default: .gray
        }
    }

    private var label: String {
        switch status {
        case let .connected(name): name
        case .searching: "Looking for the clock…"
        case .pairing: "Pairing…"
        case .notPaired: "No clock paired"
        case .bluetoothOff: "Bluetooth is off"
        case .unauthorized: "Bluetooth not allowed"
        }
    }
}
