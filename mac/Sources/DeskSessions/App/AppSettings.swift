import AppKit
import Carbon.HIToolbox
import DeskCore
import Foundation
import Observation
import Security

/// Everything the user can set, persisted in UserDefaults (and the Keychain for the cmux password).
@MainActor
@Observable
final class AppSettings {
    private let defaults = UserDefaults.standard

    var nightStart: Int { didSet { defaults.set(nightStart, forKey: "nightStart") } }
    var nightEnd: Int { didSet { defaults.set(nightEnd, forKey: "nightEnd") } }
    var flipScreen: Bool { didSet { defaults.set(flipScreen, forKey: "flipScreen") } }
    var swapButtons: Bool { didSet { defaults.set(swapButtons, forKey: "swapButtons") } }
    /// Percent.
    var dayBrightness: Int { didSet { defaults.set(dayBrightness, forKey: "dayBrightness") } }
    var nightBrightness: Int { didSet { defaults.set(nightBrightness, forKey: "nightBrightness") } }
    var allowPatterns: [String] { didSet { defaults.set(allowPatterns, forKey: "allowPatterns") } }
    var denyPatterns: [String] { didSet { defaults.set(denyPatterns, forKey: "denyPatterns") } }
    var hotKey: HotKey.Combo { didSet { hotKey.save(to: defaults) } }

    var cmuxPassword: String {
        didSet { Keychain.set(cmuxPassword, account: "cmux-socket") }
    }

    var filter: SessionFilter { SessionFilter(allow: allowPatterns, deny: denyPatterns) }

    init() {
        defaults.register(defaults: ["nightStart": 23, "nightEnd": 8, "dayBrightness": 100, "nightBrightness": 10])
        nightStart = defaults.integer(forKey: "nightStart")
        nightEnd = defaults.integer(forKey: "nightEnd")
        flipScreen = defaults.bool(forKey: "flipScreen")
        swapButtons = defaults.bool(forKey: "swapButtons")
        dayBrightness = defaults.integer(forKey: "dayBrightness")
        nightBrightness = defaults.integer(forKey: "nightBrightness")
        allowPatterns = defaults.stringArray(forKey: "allowPatterns") ?? []
        denyPatterns = defaults.stringArray(forKey: "denyPatterns") ?? []
        hotKey = HotKey.Combo(from: defaults) ?? .default
        cmuxPassword = Keychain.get(account: "cmux-socket") ?? ""
    }

    /// The clock's share of the settings, as protocol flags.
    var flags: Wire.Flags {
        var flags: Wire.Flags = []
        if flipScreen { flags.insert(.flipScreen) }
        if swapButtons { flags.insert(.swapButtons) }
        return flags
    }
}

/// Minimal generic-password Keychain access.
enum Keychain {
    private static let service = "DeskSessions"

    static func get(account: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
            kSecAttrAccount as String: account, kSecReturnData as String: true,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data
        else { return nil }
        return String(decoding: data, as: UTF8.self)
    }

    static func set(_ value: String, account: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(query as CFDictionary)
        guard !value.isEmpty else { return }
        var item = query
        item[kSecValueData as String] = Data(value.utf8)
        SecItemAdd(item as CFDictionary, nil)
    }
}
