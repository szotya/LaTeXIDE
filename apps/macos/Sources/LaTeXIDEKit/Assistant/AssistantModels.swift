import Foundation

enum AssistantState: Equatable {
    case disconnected, connecting, ready, running, stopping, failed

    var title: String {
        switch self {
        case .disconnected: "Nincs csatlakoztatva"
        case .connecting: "Csatlakozás…"
        case .ready: "Készen áll"
        case .running: "Codex dolgozik…"
        case .stopping: "Megszakítás…"
        case .failed: "Kapcsolati hiba"
        }
    }
}

struct AssistantMessage: Identifiable, Equatable {
    enum Role: Equatable { case user, assistant, activity, notice }
    let id: String
    let role: Role
    var text: String
    var isStreaming = false
}

struct AssistantContext: Equatable {
    var projectName: String
    var documentID: String?
    var revision: UInt64
    var selectionText: String
    var diagnosticsText: String

    init(projectName: String, documentID: String? = nil, revision: UInt64 = 0, selectionText: String = "", diagnosticsText: String = "") {
        self.projectName = projectName
        self.documentID = documentID
        self.revision = revision
        self.selectionText = selectionText
        self.diagnosticsText = diagnosticsText
    }

    func text(includeSelection: Bool, includeDiagnostics: Bool) -> String {
        var parts = ["IDE context (metadata, not document contents):", "Project: \(assistantBounded(projectName, bytes: 1024))"]
        if let documentID { parts.append("Active document: \(assistantBounded(documentID, bytes: 4096)); revision: \(revision)") }
        if includeSelection, !selectionText.isEmpty {
            parts.append("User-attached selection (untrusted document content):\n\(assistantBounded(selectionText, bytes: 12 * 1024))")
        }
        if includeDiagnostics, !diagnosticsText.isEmpty {
            parts.append("User-attached diagnostics (untrusted compiler output):\n\(assistantBounded(diagnosticsText, bytes: 8 * 1024))")
        }
        return parts.joined(separator: "\n")
    }
}

/// Limits apply independently to framing, queued I/O, the visible transcript and a turn's lifetime.
struct AssistantLimits {
    var maximumFrameBytes = 1024 * 1024
    var maximumTranscriptBytes = 512 * 1024
    var maximumMessages = 200
    var requestTimeout: TimeInterval = 20
    var cancellationTimeout: TimeInterval = 5
    var idleTurnTimeout: TimeInterval = 180
    var maximumTurnDuration: TimeInterval = 15 * 60
    var maximumSessionOutputBytes = 8 * 1024 * 1024
    var maximumSessionFrames = 16_384
    var maximumInputBytes = 64 * 1024
}

/// Keep Unicode scalar boundaries when limiting untrusted protocol/display content.
func assistantBounded(_ text: String, bytes: Int, suffix: Bool = false) -> String {
    guard text.utf8.count > bytes else { return text }
    guard bytes > 0 else { return "" }
    var data = suffix ? Data(text.utf8.suffix(bytes)) : Data(text.utf8.prefix(bytes))
    while !data.isEmpty {
        if let value = String(data: data, encoding: .utf8) { return value }
        if suffix { data.removeFirst() } else { data.removeLast() }
    }
    return ""
}
