import XCTest
import Darwin
@testable import LaTeXIDEKit

/// Real child processes exercise the public session boundary, not a mocked transport.
final class AssistantSessionTests: XCTestCase {
    @MainActor func testIDEOnlyApprovalGrantAllowsMCPWithoutEnablingUnlistedTools() async throws {
        let fixture = try AssistantFixture(scenario: "mcp-approval")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Olvasd a main.tex-et", context: "")
        try await eventually { session.state == .ready }
        XCTAssertNil(session.error)
        XCTAssertTrue(session.messages.contains { $0.text == "Árvíz 😀 kész." })
    }

    @MainActor func testHandshakeThenStreamingTurnUsesReadOnlyIDESession() async throws {
        let fixture = try AssistantFixture(scenario: "stream")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        XCTAssertTrue(session.messages.isEmpty, "Connect must not generate a model turn")
        session.send(text: "Segíts a képlettel", context: "main.tex r7")
        try await eventually { session.state == .ready && session.messages.contains { $0.text == "Árvíz 😀 kész." } }
        XCTAssertNil(session.error)
        XCTAssertEqual(session.modelName, "configured-fixture-model")
        XCTAssertTrue(session.messages.contains { $0.role == .activity && $0.text.contains("read_document") })
        XCTAssertFalse(session.messages.contains { $0.isStreaming })
    }

    @MainActor func testCancelBeforeTurnIDInterruptsActualChildRequest() async throws {
        let fixture = try AssistantFixture(scenario: "cancel")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Várj", context: "")
        try await eventually { session.state == .running && session.modelName != nil }
        session.cancel()
        XCTAssertEqual(session.state, .stopping)
        try await eventually { session.state == .ready }
        XCTAssertNil(session.error)
        XCTAssertFalse(session.messages.contains { $0.text == "late-after-cancel" })
        XCTAssertTrue(session.messages.contains { $0.role == .notice })
    }

    @MainActor func testProjectChangeDisconnectsAndDiscardsOldTranscript() async throws {
        let fixture = try AssistantFixture(scenario: "late")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Régi projekt", context: "")
        let nextRoot = fixture.root.appendingPathComponent("other-project", isDirectory: true)
        try FileManager.default.createDirectory(at: nextRoot, withIntermediateDirectories: true)
        session.configure(root: nextRoot, sessionPath: fixture.descriptor.path, bridgeURL: fixture.executable)
        try await Task.sleep(nanoseconds: 250_000_000)
        XCTAssertEqual(session.state, .disconnected)
        XCTAssertTrue(session.messages.isEmpty)
        XCTAssertNil(session.error)
    }

    @MainActor func testUnexpectedApprovalAndUnknownServerRequestsReceiveRefusals() async throws {
        let fixture = try AssistantFixture(scenario: "approval")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Teszt", context: "")
        try await eventually { session.state == .ready && session.messages.contains { $0.text == "requests-refused" } }
        XCTAssertTrue(session.messages.contains { $0.role == .notice })
        XCTAssertNil(session.error)
    }

    @MainActor func testPartialFrameEOFIsFailure() async throws {
        try await assertFailure(scenario: "partial", containing: "JSONL")
    }

    @MainActor func testInvalidUTF8IsFailure() async throws {
        try await assertFailure(scenario: "invalid", containing: "JSON")
    }

    @MainActor func testOversizedFrameIsFailure() async throws {
        try await assertFailure(scenario: "oversized", containing: "keret", limits: .init(maximumFrameBytes: 1024))
    }

    @MainActor func testHandshakeTimeoutStopsUnresponsiveChild() async throws {
        try await assertFailure(scenario: "silent", containing: "idő", limits: .init(requestTimeout: 0.15))
    }

    @MainActor func testTurnFailureSurfacesServerError() async throws {
        let fixture = try AssistantFixture(scenario: "failure")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Teszt", context: "")
        try await eventually { session.error != nil }
        XCTAssertEqual(session.state, .ready)
        XCTAssertTrue(session.error?.contains("fixture authentication failed") == true)
    }

    @MainActor func testTranscriptIsBoundedWithoutBreakingStream() async throws {
        let fixture = try AssistantFixture(scenario: "long")
        defer { fixture.remove() }
        let session = fixture.session(limits: .init(maximumTranscriptBytes: 1024, maximumMessages: 8))
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Teszt", context: "")
        try await eventually { session.state == .ready && session.messages.contains { $0.role == .assistant } }
        XCTAssertLessThanOrEqual(session.messages.reduce(0) { $0 + $1.text.utf8.count }, 1024)
        XCTAssertLessThanOrEqual(session.messages.count, 8)
        XCTAssertNil(session.error)
        XCTAssertFalse(session.messages.contains { $0.isStreaming })
    }

    @MainActor func testNewConversationResetsTranscriptWithoutSendingTurn() async throws {
        let fixture = try AssistantFixture(scenario: "stream")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Első", context: "")
        try await eventually { session.state == .ready && session.messages.count > 1 }
        session.newConversation()
        XCTAssertEqual(session.state, .ready)
        XCTAssertTrue(session.messages.isEmpty)
        session.send(text: "Második", context: "")
        try await eventually { session.state == .ready && session.messages.contains { $0.text == "Árvíz 😀 kész." } }
        XCTAssertFalse(session.messages.contains { $0.text == "Első" })
    }

    @MainActor func testWrongProjectDescriptorIsRejectedBeforeLaunch() async throws {
        let fixture = try AssistantFixture(scenario: "stream")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        let wrongRoot = fixture.root.appendingPathComponent("different-project", isDirectory: true)
        try FileManager.default.createDirectory(at: wrongRoot, withIntermediateDirectories: true)
        session.configure(root: wrongRoot, sessionPath: fixture.descriptor.path, bridgeURL: fixture.executable)
        session.connect(executable: fixture.executable)
        XCTAssertEqual(session.state, .failed)
        XCTAssertTrue(session.error?.contains("projekt") == true)
    }

    @MainActor func testPublicDescriptorIsRejectedBeforeLaunch() async throws {
        let fixture = try AssistantFixture(scenario: "stream")
        defer { fixture.remove() }
        try FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: fixture.descriptor.path)
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        XCTAssertEqual(session.state, .failed)
        XCTAssertTrue(session.error?.contains("privát") == true)
    }

    @MainActor func testMalformedConfigurationIsRejectedBeforeThreadStarts() async throws {
        let fixture = try AssistantFixture(scenario: "malformed-config")
        defer { fixture.remove() }
        let session = fixture.session()
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .ready }
        session.send(text: "Teszt", context: "")
        try await eventually { session.state == .failed }
        XCTAssertTrue(session.error?.contains("konfiguráció") == true, session.error ?? "Missing error")
    }

    @MainActor func testDroppingSessionStopsOwnedChild() async throws {
        let fixture = try AssistantFixture(scenario: "stream")
        defer { fixture.remove() }
        var session: AssistantSession? = fixture.session()
        session?.connect(executable: fixture.executable)
        try await eventually { session?.state == .ready }
        let pid = try XCTUnwrap(Int32(try XCTUnwrap(session?.serverVersion?.split(separator: "=").last).description))
        defer { if kill(pid, 0) == 0 { _ = kill(pid, SIGKILL) } }
        weak var released = session
        session = nil
        XCTAssertNil(released)
        try await eventually { kill(pid, 0) != 0 }
    }

    @MainActor func testInstalledCodexInitializeOnly() async throws {
        guard let path = ProcessInfo.processInfo.environment["LATEXIDE_CODEX_HANDSHAKE"], !path.isEmpty else {
            throw XCTSkip("Opt-in local CLI initialize/initialized-only check; no model/thread request")
        }
        let fixture = try AssistantFixture(scenario: "stream")
        defer { fixture.remove() }
        let session = fixture.session(limits: .init(requestTimeout: 15))
        defer { session.disconnect() }
        session.connect(executable: URL(fileURLWithPath: path))
        try await eventually(timeout: 20) { session.state == .ready || session.state == .failed }
        XCTAssertEqual(session.state, .ready, session.error ?? "CLI handshake did not complete")
        XCTAssertNotNil(session.serverVersion)
        XCTAssertTrue(session.messages.isEmpty)
        XCTAssertNil(session.modelName)
        print("INITIALIZE_ONLY_CLI_RESULT: \(session.serverVersion ?? session.error ?? "missing")")
    }

    @MainActor private func assertFailure(scenario: String, containing: String, limits: AssistantLimits = .init()) async throws {
        let fixture = try AssistantFixture(scenario: scenario)
        defer { fixture.remove() }
        let session = fixture.session(limits: limits)
        defer { session.disconnect() }
        session.connect(executable: fixture.executable)
        try await eventually { session.state == .failed }
        XCTAssertTrue(session.error?.localizedCaseInsensitiveContains(containing) == true, session.error ?? "Missing error")
        XCTAssertFalse(session.canCancel)
    }

    @MainActor private func eventually(timeout: TimeInterval = 4, _ condition: @escaping @MainActor () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition(), Date() < deadline { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(condition(), "Assistant condition did not become true before timeout")
        if !condition() { throw NSError(domain: "AssistantFixtureTimeout", code: 1) }
    }
}

private struct AssistantFixture {
    let root: URL
    let executable: URL
    let descriptor: URL

    init(scenario: String) throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("latexide-assistant-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        executable = root.appendingPathComponent("codex fixture")
        descriptor = root.appendingPathComponent("private-session.json")
        let binding: [String: Any] = ["version": 1, "project_root": root.resolvingSymlinksInPath().path, "session_id": "controlled-fixture", "token": "not-a-real-session-token", "endpoint": root.appendingPathComponent("fixture.sock").path]
        try JSONSerialization.data(withJSONObject: binding).write(to: descriptor)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: descriptor.path)
        let fixturePath = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("evidence/intelligence-2026-10-31/assistant-fixture.py")
        let source = try String(contentsOf: fixturePath, encoding: .utf8)
        try ("#!/usr/bin/python3\nSCENARIO = \"\(scenario)\"\n" + source).write(to: executable, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: executable.path)
    }
    @MainActor func session(limits: AssistantLimits = .init()) -> AssistantSession {
        let session = AssistantSession(limits: limits)
        session.configure(root: root, sessionPath: descriptor.path, bridgeURL: executable)
        return session
    }
    func remove() { try? FileManager.default.removeItem(at: root) }
}
