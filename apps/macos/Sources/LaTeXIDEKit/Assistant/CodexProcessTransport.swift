import Foundation
import Darwin

/// Foundation Process + bounded, nonblocking JSONL pipes. No shell interpolation or global signal changes.
final class CodexProcessTransport {
    enum Event {
        case message([String: Any])
        case failure(String)
    }

    private let queue = DispatchQueue(label: "latexide.codex.stdio", qos: .userInitiated)
    private let process = Process()
    private let input = Pipe(), output = Pipe(), errors = Pipe()
    private let limits: AssistantLimits
    private let receive: (Event) -> Void
    private var outputSource: DispatchSourceRead?
    private var errorSource: DispatchSourceRead?
    private var writeSource: DispatchSourceWrite?
    private var buffer = Data()
    private var pending = Data()
    private var pendingOffset = 0
    private var writeGeneration = 0
    private var outputBytes = 0, errorBytes = 0, frames = 0
    private var stopped = false

    init(limits: AssistantLimits, receive: @escaping (Event) -> Void) {
        self.limits = limits
        self.receive = receive
    }

    func start(executable: URL, root: URL) throws {
        process.executableURL = executable
        process.arguments = ["app-server", "--listen", "stdio://"]
        process.currentDirectoryURL = root
        process.standardInput = input
        process.standardOutput = output
        process.standardError = errors
        for handle in [input.fileHandleForWriting, output.fileHandleForReading, errors.fileHandleForReading] {
            let flags = fcntl(handle.fileDescriptor, F_GETFL)
            guard flags >= 0, fcntl(handle.fileDescriptor, F_SETFL, flags | O_NONBLOCK) >= 0 else {
                throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
            }
        }
        // Suppress SIGPIPE for this pipe only: a child EOF must never terminate the IDE host.
        guard fcntl(input.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1) >= 0 else {
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
        }
        try process.run()
        try? input.fileHandleForReading.close()
        try? output.fileHandleForWriting.close()
        try? errors.fileHandleForWriting.close()
        queue.async { [self] in
            guard !stopped else { return }
            let stdout = DispatchSource.makeReadSource(fileDescriptor: output.fileHandleForReading.fileDescriptor, queue: queue)
            stdout.setEventHandler { [weak self] in self?.readStdout() }
            stdout.setCancelHandler { [output] in try? output.fileHandleForReading.close() }
            outputSource = stdout
            let stderr = DispatchSource.makeReadSource(fileDescriptor: errors.fileHandleForReading.fileDescriptor, queue: queue)
            stderr.setEventHandler { [weak self] in self?.readStderr() }
            stderr.setCancelHandler { [errors] in try? errors.fileHandleForReading.close() }
            errorSource = stderr
            stdout.resume()
            stderr.resume()
        }
    }

    func send(_ object: [String: Any]) {
        do {
            var data = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
            data.append(0x0A)
            let payload = data
            queue.async { [self] in
                guard !stopped else { return }
                guard pending.count - pendingOffset + payload.count <= 256 * 1024 else {
                    fail("A Codex bemeneti puffer megtelt."); return
                }
                if pendingOffset > 0 { pending.removeFirst(pendingOffset); pendingOffset = 0 }
                let wasEmpty = pending.isEmpty
                pending.append(payload)
                if wasEmpty {
                    writeGeneration += 1
                    let generation = writeGeneration
                    queue.asyncAfter(deadline: .now() + 3) { [weak self] in
                        guard let self, !self.stopped, self.writeGeneration == generation, !self.pending.isEmpty else { return }
                        self.fail("A Codex nem olvasta a bemenetet az időkorláton belül.")
                    }
                }
                flush()
            }
        } catch {
            queue.async { [self] in fail("A Codex-kérés nem alakítható JSON-ná.") }
        }
    }

    func stop() { queue.async { [self] in close() } }

    private func flush() {
        guard !stopped else { return }
        while pendingOffset < pending.count {
            let count = pending.withUnsafeBytes { bytes in
                Darwin.write(input.fileHandleForWriting.fileDescriptor, bytes.baseAddress!.advanced(by: pendingOffset), bytes.count - pendingOffset)
            }
            if count > 0 { pendingOffset += count; continue }
            if count < 0, errno == EINTR { continue }
            if count < 0, errno == EAGAIN || errno == EWOULDBLOCK {
                if writeSource == nil {
                    let source = DispatchSource.makeWriteSource(fileDescriptor: input.fileHandleForWriting.fileDescriptor, queue: queue)
                    source.setEventHandler { [weak self] in self?.flush() }
                    writeSource = source
                    source.resume()
                }
                return
            }
            fail("A Codex bemeneti kapcsolata lezárult."); return
        }
        pending.removeAll(keepingCapacity: false)
        pendingOffset = 0
        writeGeneration += 1
        writeSource?.cancel()
        writeSource = nil
    }

    private func readStdout() {
        guard !stopped else { return }
        var chunk = [UInt8](repeating: 0, count: 16 * 1024)
        for _ in 0..<8 {
            let count = Darwin.read(output.fileHandleForReading.fileDescriptor, &chunk, chunk.count)
            if count < 0, errno == EINTR { continue }
            if count < 0, errno == EAGAIN || errno == EWOULDBLOCK { return }
            if count == 0 {
                fail(buffer.isEmpty ? "A Codex folyamat kapcsolata lezárult (EOF)." : "A Codex csonka JSONL-kerettel zárta a kapcsolatot.")
                return
            }
            guard count > 0 else { fail("A Codex kimenete nem olvasható."); return }
            outputBytes += count
            guard outputBytes <= limits.maximumSessionOutputBytes else { fail("A Codex elérte a munkamenet kimeneti korlátját. Nyiss új beszélgetést."); return }
            buffer.append(contentsOf: chunk.prefix(count))
            while let newline = buffer.firstIndex(of: 0x0A) {
                let length = buffer.distance(from: buffer.startIndex, to: newline)
                guard length <= limits.maximumFrameBytes else { fail("A Codex JSONL-keret túl nagy."); return }
                let frame = Data(buffer[..<newline])
                buffer.removeSubrange(...newline)
                if frame.isEmpty { continue }
                frames += 1
                guard frames <= limits.maximumSessionFrames else { fail("A Codex túl sok protokollüzenetet küldött. Nyiss új beszélgetést."); return }
                guard String(data: frame, encoding: .utf8) != nil,
                      let object = try? JSONSerialization.jsonObject(with: frame) as? [String: Any] else {
                    fail("A Codex érvénytelen UTF-8/JSON választ küldött."); return
                }
                receive(.message(object))
            }
            guard buffer.count <= limits.maximumFrameBytes else { fail("A Codex JSONL-keret túl nagy."); return }
        }
    }

    private func readStderr() {
        guard !stopped else { return }
        var chunk = [UInt8](repeating: 0, count: 16 * 1024)
        for _ in 0..<8 {
            let count = Darwin.read(errors.fileHandleForReading.fileDescriptor, &chunk, chunk.count)
            if count < 0, errno == EINTR { continue }
            if count < 0, errno == EAGAIN || errno == EWOULDBLOCK { return }
            if count <= 0 { errorSource?.cancel(); errorSource = nil; return }
            // Drain without publishing or persisting arbitrary CLI logs, which may contain credentials.
            errorBytes += count
            if errorBytes > 1024 * 1024 { fail("A Codex hibakimenete elérte a korlátot."); return }
        }
    }

    private func fail(_ message: String) {
        guard !stopped else { return }
        receive(.failure(message))
        close()
    }

    private func close() {
        guard !stopped else { return }
        stopped = true
        outputSource?.cancel(); outputSource = nil
        errorSource?.cancel(); errorSource = nil
        writeSource?.cancel(); writeSource = nil
        try? input.fileHandleForWriting.close()
        pending.removeAll(); buffer.removeAll()
        if process.isRunning { process.terminate() }
        // Do not let an unresponsive child survive disconnect/cancel indefinitely.
        let child = process
        queue.asyncAfter(deadline: .now() + 2) {
            if child.isRunning { _ = kill(child.processIdentifier, SIGKILL) }
        }
    }
}
