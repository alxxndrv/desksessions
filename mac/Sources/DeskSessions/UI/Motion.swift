import SwiftUI

/// Every animation in the Mac app comes from here. Short and crisp, Raycast-fast: the panel is
/// a tool you flick open, so nothing should make you wait for it.
enum Motion {
    /// Things moving to a new place: rows reordering, list changes, state changes.
    static let snappy = Animation.snappy(duration: 0.22, extraBounce: 0.04)
    /// The selection highlight following the keyboard.
    static let select = Animation.smooth(duration: 0.12)
    /// Scrolling the list to keep the selection in view.
    static let scroll = Animation.smooth(duration: 0.14)
    /// The panel springing in under the menu bar.
    static let appear = Animation.snappy(duration: 0.2, extraBounce: 0.06)
    /// The panel going away.
    static let disappear = Animation.easeIn(duration: 0.08)
    /// Seconds the panel takes to fade before it is ordered out (matches `disappear`).
    static let disappearDelay = 0.09
    /// Slow ambient pulses (NEEDS YOU bar, searching dot) stay slow on purpose.
    static let pulse = Animation.easeInOut(duration: 0.7)
}
