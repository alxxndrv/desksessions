import Darwin
import Foundation

/// The Unix socket Codex hooks report to: one JSON line per event.
public enum HookSocket {
    public static var path: String {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/DeskSessions/hook.sock").path
    }

    /// What the hook sends: the fields of Codex's payload we use (tool inputs can be huge and
    /// are none of our business), the cmux variables, and the hook's parent process.
    public static func envelope(stdin: Data, environment: [String: String], parentPid: Int32) -> Data? {
        guard let payload = try? JSONSerialization.jsonObject(with: stdin) as? [String: Any] else {
            return nil
        }
        let keep = ["hook_event_name", "session_id", "cwd", "tool_name", "agent_id", "turn_id"]
        let trimmed = payload.filter { keep.contains($0.key) }
        let env = environment.filter { $0.key == "CMUX_SURFACE_ID" || $0.key == "CMUX_WORKSPACE_ID" }
        let object: [String: Any] = ["payload": trimmed, "env": env, "ppid": Int(parentPid)]
        guard var line = try? JSONSerialization.data(withJSONObject: object) else { return nil }
        line.append(0x0A)
        return line
    }

    /// Sends one line and gives up quickly: a hook must never slow Codex down.
    @discardableResult
    public static func send(_ line: Data, to path: String = path) -> Bool {
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { close(fd) }
        var timeout = timeval(tv_sec: 0, tv_usec: 200_000)
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        var noSigPipe: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noSigPipe, socklen_t(MemoryLayout<Int32>.size))
        guard var address = unixAddress(path) else { return false }
        let connected = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else { return false }
        return line.withUnsafeBytes { write(fd, $0.baseAddress, line.count) } == line.count
    }

    static func unixAddress(_ path: String) -> sockaddr_un? {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: address.sun_path) else { return nil }
        withUnsafeMutableBytes(of: &address.sun_path) { raw in
            raw.copyBytes(from: bytes)
            raw[bytes.count] = 0
        }
        return address
    }
}

/// Listens on the hook socket and hands each received line to `onLine` on `queue`.
public final class HookServer: @unchecked Sendable {
    private let path: String
    private let queue: DispatchQueue
    private let onLine: @Sendable (Data) -> Void
    private var listener: Int32 = -1
    private var acceptSource: DispatchSourceRead?

    public init(path: String = HookSocket.path, queue: DispatchQueue,
                onLine: @escaping @Sendable (Data) -> Void) {
        self.path = path
        self.queue = queue
        self.onLine = onLine
    }

    deinit { stop() }

    public func start() throws {
        try FileManager.default.createDirectory(
            atPath: (path as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
        unlink(path)
        listener = socket(AF_UNIX, SOCK_STREAM, 0)
        guard listener >= 0, var address = HookSocket.unixAddress(path) else {
            throw POSIXError(.EINVAL)
        }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, listen(listener, 16) == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        chmod(path, 0o600)
        _ = fcntl(listener, F_SETFL, O_NONBLOCK)

        let source = DispatchSource.makeReadSource(fileDescriptor: listener, queue: queue)
        source.setEventHandler { [weak self] in self?.acceptAll() }
        source.resume()
        acceptSource = source
    }

    public func stop() {
        acceptSource?.cancel()
        acceptSource = nil
        if listener >= 0 {
            close(listener)
            listener = -1
            unlink(path)
        }
    }

    private func acceptAll() {
        while true {
            let client = accept(listener, nil, nil)
            guard client >= 0 else { return }
            read(client)
        }
    }

    /// Hooks send one short line and close, so reading to EOF with a short timeout is enough.
    private func read(_ client: Int32) {
        var timeout = timeval(tv_sec: 0, tv_usec: 300_000)
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while data.count < 64 * 1024 {
            let n = Darwin.read(client, &buffer, buffer.count)
            if n <= 0 { break }
            data.append(buffer, count: n)
        }
        close(client)
        for line in data.split(separator: 0x0A) where !line.isEmpty {
            onLine(Data(line))
        }
    }
}
