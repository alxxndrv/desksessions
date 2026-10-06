// Runs on every Codex hook event: forwards the event to the DeskSessions app and exits.
// It prints nothing and always succeeds, so it can never change what Codex does.

import Darwin
import DeskCore
import Foundation

let input = FileHandle.standardInput.readDataToEndOfFile()
if let line = HookSocket.envelope(
    stdin: input, environment: ProcessInfo.processInfo.environment, parentPid: getppid()) {
    HookSocket.send(line)
}
exit(0)
