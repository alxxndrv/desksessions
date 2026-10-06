import Darwin
import Foundation
import Testing

@testable import DeskCore

@Test func readsOurOwnProcess() {
    let me = getpid()
    #expect(Processes.isAlive(me))
    #expect(!Processes.isAlive(999_999))
    #expect(Processes.executablePath(of: me)?.isEmpty == false)
    #expect(Processes.lineage(of: me).count > 1)
    #expect(Processes.environment(of: me)["PATH"] == ProcessInfo.processInfo.environment["PATH"])
}
