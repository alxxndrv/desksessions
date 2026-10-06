import AppKit
import Carbon.HIToolbox

/// A system-wide shortcut through Carbon's RegisterEventHotKey (no Accessibility permission needed).
@MainActor
final class HotKey {
    struct Combo: Equatable {
        var keyCode: UInt32
        var modifiers: NSEvent.ModifierFlags

        static let `default` = Combo(keyCode: UInt32(kVK_ANSI_S), modifiers: [.option, .command])

        init(keyCode: UInt32, modifiers: NSEvent.ModifierFlags) {
            self.keyCode = keyCode
            self.modifiers = modifiers.intersection([.command, .option, .control, .shift])
        }

        init?(from defaults: UserDefaults) {
            guard defaults.object(forKey: "hotKeyCode") != nil else { return nil }
            self.init(keyCode: UInt32(defaults.integer(forKey: "hotKeyCode")),
                      modifiers: NSEvent.ModifierFlags(rawValue: UInt(defaults.integer(forKey: "hotKeyModifiers"))))
        }

        func save(to defaults: UserDefaults) {
            defaults.set(Int(keyCode), forKey: "hotKeyCode")
            defaults.set(Int(modifiers.rawValue), forKey: "hotKeyModifiers")
        }

        var carbonModifiers: UInt32 {
            var flags: UInt32 = 0
            if modifiers.contains(.command) { flags |= UInt32(cmdKey) }
            if modifiers.contains(.option) { flags |= UInt32(optionKey) }
            if modifiers.contains(.control) { flags |= UInt32(controlKey) }
            if modifiers.contains(.shift) { flags |= UInt32(shiftKey) }
            return flags
        }

        /// "⌥⌘S"
        var display: String {
            var text = ""
            if modifiers.contains(.control) { text += "⌃" }
            if modifiers.contains(.option) { text += "⌥" }
            if modifiers.contains(.shift) { text += "⇧" }
            if modifiers.contains(.command) { text += "⌘" }
            return text + Self.keyName(keyCode)
        }

        static func keyName(_ code: UInt32) -> String {
            let source = TISCopyCurrentKeyboardLayoutInputSource().takeRetainedValue()
            guard let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else { return "?" }
            let layout = unsafeBitCast(raw, to: CFData.self) as Data
            var deadKeys: UInt32 = 0
            var chars = [UniChar](repeating: 0, count: 4)
            var length = 0
            let status = layout.withUnsafeBytes { bytes in
                UCKeyTranslate(bytes.bindMemory(to: UCKeyboardLayout.self).baseAddress, UInt16(code),
                               UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                               OptionBits(kUCKeyTranslateNoDeadKeysBit), &deadKeys, 4, &length, &chars)
            }
            guard status == noErr, length > 0 else { return "?" }
            return String(utf16CodeUnits: chars, count: length).uppercased()
        }
    }

    private var reference: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let action: () -> Void

    init(action: @escaping () -> Void) {
        self.action = action
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, _, context in
            guard let context else { return noErr }
            let hotKey = Unmanaged<HotKey>.fromOpaque(context).takeUnretainedValue()
            MainActor.assumeIsolated { hotKey.action() }
            return noErr
        }, 1, &spec, Unmanaged.passUnretained(self).toOpaque(), &handler)
    }

    func register(_ combo: Combo) {
        if let reference { UnregisterEventHotKey(reference) }
        let id = EventHotKeyID(signature: OSType(0x6453_4B53), id: 1)  // 'dSKS'
        RegisterEventHotKey(combo.keyCode, combo.carbonModifiers, id, GetApplicationEventTarget(), 0, &reference)
    }
}
