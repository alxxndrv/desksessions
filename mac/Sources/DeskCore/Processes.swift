import Darwin
import Foundation

/// Facts about other processes of the same user.
public enum Processes {
    public static func isAlive(_ pid: Int32) -> Bool {
        pid > 0 && (kill(pid, 0) == 0 || errno == EPERM)
    }

    public static func parent(of pid: Int32) -> Int32? {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, pid]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0, size > 0 else { return nil }
        let ppid = info.kp_eproc.e_ppid
        return ppid > 1 ? ppid : nil
    }

    public static func executablePath(of pid: Int32) -> String? {
        var buffer = [CChar](repeating: 0, count: 4 * Int(MAXPATHLEN))
        let length = proc_pidpath(pid, &buffer, UInt32(buffer.count))
        guard length > 0 else { return nil }
        return String(decoding: buffer.prefix(Int(length)).map { UInt8(bitPattern: $0) }, as: UTF8.self)
    }

    /// The process and its ancestors, nearest first.
    public static func lineage(of pid: Int32, limit: Int = 12) -> [Int32] {
        var chain: [Int32] = [pid]
        var current = pid
        while chain.count < limit, let next = parent(of: current) {
            chain.append(next)
            current = next
        }
        return chain
    }

    /// The process's environment, from its argument block (KERN_PROCARGS2).
    public static func environment(of pid: Int32) -> [String: String] {
        var mib: [Int32] = [CTL_KERN, KERN_PROCARGS2, pid]
        var size = 0
        guard sysctl(&mib, 3, nil, &size, nil, 0) == 0, size > 0 else { return [:] }
        var buffer = [UInt8](repeating: 0, count: size)
        guard sysctl(&mib, 3, &buffer, &size, nil, 0) == 0, size > MemoryLayout<Int32>.size else {
            return [:]
        }
        // Layout: argc, executable path, padding NULs, argv[argc], then environment strings.
        let argc = buffer.withUnsafeBytes { $0.load(as: Int32.self) }
        var index = MemoryLayout<Int32>.size
        func skipString() {
            while index < size && buffer[index] != 0 { index += 1 }
        }
        func skipNuls() {
            while index < size && buffer[index] == 0 { index += 1 }
        }
        skipString()
        skipNuls()
        for _ in 0..<argc {
            skipString()
            index += 1
        }
        var env: [String: String] = [:]
        while index < size {
            let start = index
            skipString()
            if index == start { break }
            if let entry = String(bytes: buffer[start..<index], encoding: .utf8),
               let eq = entry.firstIndex(of: "=") {
                env[String(entry[..<eq])] = String(entry[entry.index(after: eq)...])
            }
            index += 1
        }
        return env
    }

    /// Where a terminal session lives, from the cmux variables in its environment.
    public static func cmuxHome(of pid: Int32) -> SessionHome {
        let env = environment(of: pid)
        guard let surface = env["CMUX_SURFACE_ID"] else { return .unknown }
        return .cmux(workspace: env["CMUX_WORKSPACE_ID"], surface: surface)
    }
}
