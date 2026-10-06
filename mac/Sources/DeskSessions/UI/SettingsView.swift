import DeskCore
import ServiceManagement
import SwiftUI

struct SettingsView: View {
    @Bindable var settings: AppSettings
    let link: ClockLink
    let monitor: SessionMonitor
    let hookPath: String
    let onHotKeyChange: (HotKey.Combo) -> Void

    var body: some View {
        TabView {
            ClockTab(settings: settings, link: link)
                .tabItem { Label("Clock", systemImage: "rectangle.split.3x1") }
            SessionsTab(settings: settings, monitor: monitor, hookPath: hookPath)
                .tabItem { Label("Sessions", systemImage: "list.bullet.rectangle") }
            GeneralTab(settings: settings, onHotKeyChange: onHotKeyChange)
                .tabItem { Label("General", systemImage: "gearshape") }
        }
        .frame(width: 520, height: 430)
        .scenePadding()
    }
}

private struct ClockTab: View {
    @Bindable var settings: AppSettings
    let link: ClockLink
    @State private var pairing = false

    var body: some View {
        Form {
            Section {
                HStack {
                    LinkBadge(status: link.status)
                    Spacer()
                    Button("Pair New Clock…") {
                        pairing = true
                        link.startPairing()
                    }
                    if link.status != .notPaired {
                        Button("Forget") { link.forget() }
                    }
                }
            }
            Section("Brightness") {
                BrightnessSlider(title: "Day", value: $settings.dayBrightness, symbol: "sun.max")
                BrightnessSlider(title: "Quiet hours", value: $settings.nightBrightness, symbol: "moon")
                Text("Changes show on the clock as you drag. The clock dims further on its own when nothing's happening.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Section("Orientation") {
                Toggle("Flip the screen upside down", isOn: $settings.flipScreen)
                Toggle("Swap the buttons", isOn: $settings.swapButtons)
            }
            Section("Quiet hours") {
                HStack {
                    Picker("From", selection: $settings.nightStart) { hours }
                    Picker("to", selection: $settings.nightEnd) { hours }
                }
                Text("The clock stays dim and doesn't flash.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .sheet(isPresented: $pairing, onDismiss: { link.stopPairing() }) {
            PairingSheet(link: link, done: { pairing = false })
        }
    }

    private var hours: some View {
        ForEach(0..<24, id: \.self) { Text(String(format: "%02d:00", $0)).tag($0) }
    }
}

private struct BrightnessSlider: View {
    let title: String
    @Binding var value: Int
    let symbol: String

    var body: some View {
        LabeledContent(title) {
            HStack(spacing: 10) {
                Image(systemName: symbol).foregroundStyle(.secondary).frame(width: 16)
                Slider(value: Binding(get: { Double(value) }, set: { value = Int($0.rounded()) }),
                       in: 1...100)
                Text("\(value)%")
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
                    .frame(width: 40, alignment: .trailing)
                    .contentTransition(.numericText(value: Double(value)))
                    .animation(Motion.snappy, value: value)
            }
        }
    }
}

private struct PairingSheet: View {
    let link: ClockLink
    let done: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Pair a clock").font(.title3.weight(.semibold))
            Text("Clocks showing “Let's pair” appear below. After you pick one, the clock shows a six-digit code: type it into the macOS prompt.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            List(link.nearby) { clock in
                HStack {
                    Image(systemName: "rectangle.split.3x1")
                    Text(clock.name)
                    Spacer()
                    Text("\(clock.rssi) dBm").foregroundStyle(.tertiary).font(.caption)
                    Button("Pair") {
                        link.pair(with: clock.id)
                        done()
                    }
                }
                .transition(.move(edge: .top).combined(with: .opacity))
            }
            .frame(height: 160)
            .overlay {
                if link.nearby.isEmpty {
                    ProgressView("Looking for clocks…").controlSize(.small)
                }
            }
            .animation(Motion.snappy, value: link.nearby)
            HStack {
                Spacer()
                Button("Cancel", action: done).keyboardShortcut(.cancelAction)
            }
        }
        .padding(20)
        .frame(width: 420)
    }
}

private struct SessionsTab: View {
    @Bindable var settings: AppSettings
    let monitor: SessionMonitor
    let hookPath: String
    @State private var hooks = HooksInstaller().status()
    @State private var hooksError: String?

    var body: some View {
        Form {
            Section("Codex") {
                HStack {
                    switch hooks {
                    case .installed:
                        Label("Hooks installed", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                        Spacer()
                        Button("Remove") { change { try HooksInstaller().uninstall() } }
                    case .notInstalled:
                        Label("Hooks not installed", systemImage: "circle.dashed").foregroundStyle(.secondary)
                        Spacer()
                        Button("Install") { change { try HooksInstaller().install(hookPath: hookPath) } }
                    }
                }
                Text("Adds DeskSessions to ~/.codex/hooks.json next to your other hooks. A backup is made first.")
                    .font(.caption).foregroundStyle(.secondary)
                if let error = hooksError ?? monitor.hookServerError {
                    Text(error).font(.caption).foregroundStyle(.red)
                }
            }
            Section("Folders") {
                PatternField(title: "Only show sessions in", patterns: $settings.allowPatterns)
                PatternField(title: "Never show sessions in", patterns: $settings.denyPatterns)
                Text("One glob per line, e.g. ~/code/* — empty shows everything.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Section("cmux") {
                SecureField("Socket password", text: $settings.cmuxPassword)
                Text("Only needed if cmux refuses Jump because its socket admits processes started inside cmux alone.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }

    private func change(_ action: () throws -> Void) {
        do {
            try action()
            hooksError = nil
        } catch {
            hooksError = error.localizedDescription
        }
        withAnimation(Motion.snappy) { hooks = HooksInstaller().status() }
    }
}

private struct PatternField: View {
    let title: String
    @Binding var patterns: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.callout)
            TextEditor(text: Binding(
                get: { patterns.joined(separator: "\n") },
                set: { text in
                    patterns = text.split(separator: "\n").map {
                        ($0.trimmingCharacters(in: .whitespaces) as NSString).expandingTildeInPath
                    }.filter { !$0.isEmpty }
                }))
                .font(.system(size: 12, design: .monospaced))
                .frame(height: 46)
        }
    }
}

private struct GeneralTab: View {
    @Bindable var settings: AppSettings
    let onHotKeyChange: (HotKey.Combo) -> Void
    @State private var launchAtLogin = SMAppService.mainApp.status == .enabled
    @State private var loginError: String?

    var body: some View {
        Form {
            Section {
                LabeledContent("Show panel") {
                    HotKeyRecorder(combo: $settings.hotKey) { onHotKeyChange($0) }
                }
                Toggle("Open at login", isOn: $launchAtLogin)
                    .onChange(of: launchAtLogin) { _, enabled in
                        do {
                            if enabled { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
                            loginError = nil
                        } catch {
                            loginError = error.localizedDescription
                            launchAtLogin = SMAppService.mainApp.status == .enabled
                        }
                    }
                if let loginError {
                    Text(loginError).font(.caption).foregroundStyle(.red)
                }
            }
        }
        .formStyle(.grouped)
    }
}

/// Click, then press the new shortcut.
private struct HotKeyRecorder: View {
    @Binding var combo: HotKey.Combo
    let onChange: (HotKey.Combo) -> Void
    @State private var recording = false
    @State private var monitor: Any?

    var body: some View {
        Button(recording ? "Type shortcut…" : combo.display) {
            recording ? stop() : start()
        }
        .monospaced()
        .animation(Motion.snappy, value: recording)
    }

    private func start() {
        recording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let modifiers = event.modifierFlags.intersection([.command, .option, .control, .shift])
            if event.keyCode == 53 {  // Esc cancels
                stop()
                return nil
            }
            guard !modifiers.isEmpty else { return nil }
            let next = HotKey.Combo(keyCode: UInt32(event.keyCode), modifiers: modifiers)
            combo = next
            onChange(next)
            stop()
            return nil
        }
    }

    private func stop() {
        recording = false
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}
