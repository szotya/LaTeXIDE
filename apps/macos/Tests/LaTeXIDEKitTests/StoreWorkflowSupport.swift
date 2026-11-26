import Darwin
import Foundation
import XCTest
@testable import LaTeXIDEKit

struct StoreWorkflowError: Error, CustomStringConvertible {
    let description: String
}

struct StoreWorkflowDiagnosticError: Error, CustomStringConvertible {
    let primary: Error
    let diagnostic: Error
    var description: String {
        "\(String(describing: primary)); diagnostic persistence also failed: \(String(describing: diagnostic))"
    }
}

func storeWorkflowRequire(_ condition: @autoclosure () throws -> Bool, _ message: String) throws {
    guard try condition() else { throw StoreWorkflowError(description: message) }
}

final class StoreWorkflowMemoryPreferences: EditorPreferenceStorage {
    var values: [String: Any] = [:]
    var writtenKeys: [String] = []
    func object(forKey key: String) -> Any? { values[key] }
    func string(forKey key: String) -> String? { values[key] as? String }
    func double(forKey key: String) -> Double { (values[key] as? NSNumber)?.doubleValue ?? 0 }
    func bool(forKey key: String) -> Bool { (values[key] as? NSNumber)?.boolValue ?? false }
    func set(_ value: Any?, forKey key: String) {
        values[key] = value
        writtenKeys.append(key)
    }
}


final class StoreWorkflowTrace {
    let directory: URL
    let method: String
    let nonce: String
    var events: [[String: Any]] = []
    var cleanup: [String: Any] = [:]
    var descriptor: URL?
    var endpoint: URL?
    var sessionPathObservation: [String: Any] = [:]
    var sessionCleanupIdentity: [String: String] = [:]
    var error: String?
    init(_ directory: URL, method: String) {
        self.directory = directory
        self.method = method
        self.nonce = ProcessInfo.processInfo.environment["LATEXIDE_STORE_WORKFLOW_NONCE"] ?? UUID().uuidString
    }
    @MainActor func capture(_ step: String, _ store: WorkspaceStore,
                            _ raw: SourceCharacterCount?, _ body: LatexBodyCharacterCount?) {
        events.append([
            "step": step,
            "source_utf8_hex": Data(store.text.utf8).map { String(format: "%02x", $0) }.joined(),
            "revision": store.revision, "dirty": store.dirty,
            "selection": store.selection as Any? ?? NSNull(),
            "canUndo": store.canUndo, "canRedo": store.canRedo,
            "isComposing": store.isComposing,
            "raw_total": raw.map { $0.includingWhitespace } as Any? ?? NSNull(),
            "raw_without_whitespace": raw.map { $0.excludingWhitespace } as Any? ?? NSNull(),
            "body": body?.characterCount as Any? ?? NSNull(),
            "body_status": body?.status.rawValue as Any? ?? NSNull(),
            "body_issues": body.map { $0.issues.map(\.rawValue) } as Any? ?? NSNull(),
            "body_display": body?.displayText as Any? ?? NSNull()
        ])
    }



    func recordSessionObservation(_ row: [String: Any]) throws {
        sessionPathObservation = row
        let target = directory.appendingPathComponent("session-path-diagnostic.json")
        try JSONSerialization.data(withJSONObject: row, options: [.prettyPrinted, .sortedKeys])
            .write(to: target, options: .withoutOverwriting)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: target.path)
    }
    func save() throws {
        let row: [String: Any] = [
            "method": method, "pid": getpid(), "parent_pid": getppid(), "nonce": nonce,
            "events": events, "cleanup": cleanup, "session_path_observation": sessionPathObservation,
            "error": error as Any? ?? NSNull(), "credential_values_read_or_recorded": false,
            "runloop_pump_calls": 0, "UI_initialization_calls": 0,
            "rendered_body_visibility": "UNRUN_STORE_STATE_ONLY"
        ]
        let target = directory.appendingPathComponent("workflow.json")
        try JSONSerialization.data(withJSONObject: row, options: [.prettyPrinted, .sortedKeys])
            .write(to: target, options: .withoutOverwriting)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: target.path)
    }
}
@MainActor final class StoreWorkflowCounts {
    let memory: StoreWorkflowMemoryPreferences
    let preferences: EditorPreferences
    let rawCache = SourceCharacterCountCache()
    let bodyCache = CharacterCountResultCache<LatexBodyCharacterCount>()
    init() {
        let storage = StoreWorkflowMemoryPreferences()
        memory = storage
        preferences = EditorPreferences(storage: storage)
    }

    func raw(_ store: WorkspaceStore) -> SourceCharacterCount? {
        SourceCharacterCountPresentation(preferences: preferences, cache: rawCache).count(in: store.text)
    }
    func body(_ store: WorkspaceStore) -> LatexBodyCharacterCount {
        let text = store.text
        return bodyCache.value(in: text, revision: store.revision, mode: .latexBody) {
            LatexBodyCharacterCount(text)
        }
    }
    func assert(_ store: WorkspaceStore, rawTotal: Int, rawWithoutWhitespace: Int,
                bodyCount: Int?, bodyStatus: LatexBodyCharacterCount.Status = .complete,
                issue: LatexBodyCharacterCount.Issue? = nil, step: String, trace: StoreWorkflowTrace) throws {
        let raw = self.raw(store)
        let body = self.body(store)
        try storeWorkflowRequire(raw?.includingWhitespace == rawTotal, "Wrong genuine Store raw total at \(step)")
        try storeWorkflowRequire(raw?.excludingWhitespace == rawWithoutWhitespace, "Wrong raw whitespace count at \(step)")
        try storeWorkflowRequire(body.status == bodyStatus && body.characterCount == bodyCount,
                    "Wrong lexical body result at \(step)")
        if let issue { try storeWorkflowRequire(body.issues.contains(issue), "Missing lexical refusal issue at \(step)") }
        if bodyStatus != .complete {
            try storeWorkflowRequire(body.characterCount == nil && body.displayText == (bodyStatus == .partial ? "Szövegtörzs: részleges" : "Szövegtörzs: nem támogatott"),
                        "Unsupported input exposed a precise-looking body count at \(step)")
        }
        trace.capture(step, store, raw, body)
    }
}


struct StoreWorkflowSessionBinding {
    let descriptor: URL
    let endpoint: URL
    let cleanupIdentity: [String: String]
    let legacyParentMatches: Bool
    let normalizedParentMatches: Bool
    let legacyPIDMatches: Bool
    let legacyExtensionMatches: Bool
}

// Authority comes from the repository factory contract and the current UID.
// Returned paths are observations only. No descriptor/token contents are opened.
enum StoreWorkflowSessionAuthority {
    static func normalized(_ url: URL) -> URL {
        url.standardizedFileURL.resolvingSymlinksInPath().standardizedFileURL
    }
    static func metadata(_ row: stat) -> [String: Any] {
        ["uid": row.st_uid, "mode": UInt32(row.st_mode) & 0o7777,
         "kind": UInt32(row.st_mode) & UInt32(S_IFMT),
         "dev": String(row.st_dev), "inode": String(row.st_ino), "links": row.st_nlink]
    }
    static func status(_ path: String) throws -> stat? {
        var row = stat()
        let result = path.withCString { Darwin.lstat($0, &row) }
        let failure = errno
        if result == 0 { return row }
        if failure == ENOENT { return nil }
        throw StoreWorkflowError(description: "Own Session lstat error: \(failure)")
    }
    static func noFollow(_ path: String) throws -> stat {
        guard let row = try status(path) else {
            throw StoreWorkflowError(description: "Own Session metadata missing")
        }
        return row
    }
    static func validate(_ raw: String, _ observation: inout [String: Any]) throws -> StoreWorkflowSessionBinding {
        // Contract: crates/latexide-ffi/src/session.rs, runtime_directory (Unix).
        // The fixture's temporary directory is not authority for a returned Session path.
        let uid = geteuid()
        let fixedParent = "/private/tmp/latexide-\(uid)"
        let aliasParent = "/tmp/latexide-\(uid)"
        let systemAliasAllowed = normalized(URL(fileURLWithPath: "/tmp")).path ==
            normalized(URL(fileURLWithPath: "/private/tmp")).path
        observation["expected_raw_parent"] = fixedParent
        observation["expected_euid"] = uid
        observation["system_alias_has_same_normalized_root"] = systemAliasAllowed
        observation["fixture_TMPDIR_is_Session_authority"] = false
        try storeWorkflowRequire(raw.utf8.count <= 4096 && raw.hasPrefix("/"), "Invalid Session path size/form")
        guard let slash = raw.lastIndex(of: "/") else {
            throw StoreWorkflowError(description: "Session path has no parent")
        }
        let rawParent = String(raw[..<slash])
        let name = String(raw[raw.index(after: slash)...])
        let prefix = "\(getpid())-"
        let prefixOK = name.hasPrefix(prefix)
        let suffixOK = name.hasSuffix(".json")
        let nonce = prefixOK && suffixOK ? String(name.dropFirst(prefix.count).dropLast(5)) : ""
        let fullName = prefixOK && suffixOK && nonce.utf8.count == 16 &&
            nonce.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) } &&
            name == prefix + nonce + ".json"
        let rawParentOK = rawParent == fixedParent || (rawParent == aliasParent && systemAliasAllowed)
        observation["raw_parent"] = rawParent
        observation["raw_basename"] = name
        observation["full_PID_factory_nonce_json_matches"] = fullName
        observation["raw_parent_in_closed_policy"] = rawParentOK
        // No metadata lookup or resolving of arbitrary returned paths.
        try storeWorkflowRequire(rawParentOK && fullName, "Session raw parent/PID/nonce/suffix rejected")
        let expectedRow = try noFollow(fixedParent)
        let rawParentRow = try noFollow(rawParent)
        observation["expected_parent_metadata"] = metadata(expectedRow)
        observation["returned_parent_metadata"] = metadata(rawParentRow)
        let parentsSafe = expectedRow.st_uid == uid && rawParentRow.st_uid == uid &&
            (expectedRow.st_mode & mode_t(S_IFMT)) == mode_t(S_IFDIR) &&
            (rawParentRow.st_mode & mode_t(S_IFMT)) == mode_t(S_IFDIR) &&
            (expectedRow.st_mode & 0o7777) == 0o700 && (rawParentRow.st_mode & 0o7777) == 0o700 &&
            expectedRow.st_dev == rawParentRow.st_dev && expectedRow.st_ino == rawParentRow.st_ino
        observation["physical_parent_UID_kind_mode_identity_matches"] = parentsSafe
        try storeWorkflowRequire(parentsSafe, "Session parent physical authority rejected")
        let descriptorRow = try noFollow(raw)
        observation["descriptor_metadata"] = metadata(descriptorRow)
        let descriptorSafe = descriptorRow.st_uid == uid && descriptorRow.st_nlink == 1 &&
            (descriptorRow.st_mode & mode_t(S_IFMT)) == mode_t(S_IFREG) &&
            (descriptorRow.st_mode & 0o7777) == 0o600
        observation["descriptor_UID_regular_0600_singlelink"] = descriptorSafe
        try storeWorkflowRequire(descriptorSafe, "Session descriptor metadata rejected")
        let rawURL = URL(fileURLWithPath: raw)
        let descriptor = normalized(rawURL)
        let expected = normalized(URL(fileURLWithPath: fixedParent))
        let legacyDescriptor = rawURL.standardizedFileURL
        let legacyParent = URL(fileURLWithPath: fixedParent)
        let canonicalParent = descriptor.deletingLastPathComponent().path == expected.path
        let legacyParentOK = legacyDescriptor.deletingLastPathComponent().path == legacyParent.path
        let legacyPIDOK = legacyDescriptor.lastPathComponent.hasPrefix(prefix)
        let legacyExtOK = legacyDescriptor.pathExtension == "json"
        observation["normalized_descriptor"] = descriptor.path
        observation["expected_normalized_parent"] = expected.path
        observation["legacy_standardized_descriptor"] = legacyDescriptor.path
        observation["legacy_expected_parent"] = legacyParent.path
        observation["legacy_parent_matches"] = legacyParentOK
        observation["legacy_PID_matches"] = legacyPIDOK
        observation["legacy_ext_matches"] = legacyExtOK
        observation["normalized_parent_matches"] = canonicalParent
        try storeWorkflowRequire(canonicalParent, "Session canonical parent mismatch")
        let endpoint = descriptor.deletingPathExtension().appendingPathExtension("sock")
        let endpointRow = try noFollow(endpoint.path)
        observation["endpoint_metadata"] = metadata(endpointRow)
        let endpointSafe = endpointRow.st_uid == uid &&
            (endpointRow.st_mode & mode_t(S_IFMT)) == mode_t(S_IFSOCK) &&
            (endpointRow.st_mode & 0o077) == 0
        observation["endpoint_UID_socket_owneronly"] = endpointSafe
        try storeWorkflowRequire(endpointSafe, "Session endpoint metadata rejected")
        return StoreWorkflowSessionBinding(descriptor: descriptor, endpoint: endpoint,
            cleanupIdentity: ["parent_dev": String(expectedRow.st_dev), "parent_inode": String(expectedRow.st_ino),
                              "endpoint_dev": String(endpointRow.st_dev), "endpoint_inode": String(endpointRow.st_ino)],
            legacyParentMatches: legacyParentOK, normalizedParentMatches: canonicalParent,
            legacyPIDMatches: legacyPIDOK, legacyExtensionMatches: legacyExtOK)
    }
    static func admit(_ raw: String, trace: StoreWorkflowTrace) throws -> StoreWorkflowSessionBinding {
        var observation: [String: Any] = [
            "raw": raw, "normalized_descriptor": "UNOBSERVED_UNTIL_CLOSED_RAW_AUTHORITY",
            "authority_validated": false, "credential_content_read": false
        ]
        let binding: StoreWorkflowSessionBinding
        do {
            binding = try validate(raw, &observation)
        } catch let validationError {
            observation["authority_error"] = String(describing: validationError)
            do {
                try trace.recordSessionObservation(observation)
            } catch let diagnosticError {
                throw StoreWorkflowDiagnosticError(primary: validationError, diagnostic: diagnosticError)
            }
            throw validationError
        }
        observation["authority_validated"] = true
        // Cleanup must retain validated ownership even if the diagnostic write fails.
        trace.descriptor = binding.descriptor
        trace.endpoint = binding.endpoint
        trace.sessionCleanupIdentity = binding.cleanupIdentity
        try trace.recordSessionObservation(observation)
        return binding
    }
}


enum StoreWorkflowFixture {
    static let creationMaskLock = NSLock()
    static let initialMain = "alma fa"
    static let initialSecond = "e\u{301} 👩‍💻\n"
    static func create() throws -> (directory: URL, retain: Bool) {
        let configured = ProcessInfo.processInfo.environment["LATEXIDE_STORE_WORKFLOW_ARTIFACT_ROOT"]
        let base: URL
        if let configured {
            base = URL(fileURLWithPath: configured).standardizedFileURL.resolvingSymlinksInPath()
            let row = try StoreWorkflowSessionAuthority.noFollow(base.path)
            try storeWorkflowRequire(row.st_uid == geteuid() &&
                (row.st_mode & mode_t(S_IFMT)) == mode_t(S_IFDIR) &&
                (row.st_mode & 0o7777) == 0o700, "Unsafe caller artifact directory")
        } else {
            base = FileManager.default.temporaryDirectory
        }
        let directory = base.appendingPathComponent("latexide-store-workflow-\(getpid())-\(UUID().uuidString)")
            .standardizedFileURL.resolvingSymlinksInPath()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
                                               attributes: [.posixPermissions: 0o700])
        let owner = try StoreWorkflowSessionAuthority.noFollow(directory.path)
        try storeWorkflowRequire(owner.st_uid == geteuid() &&
            (owner.st_mode & mode_t(S_IFMT)) == mode_t(S_IFDIR) &&
            (owner.st_mode & 0o7777) == 0o700, "Unsafe private workflow fixture")
        let project = directory.appendingPathComponent("project")
        try FileManager.default.createDirectory(at: project, withIntermediateDirectories: false,
                                               attributes: [.posixPermissions: 0o700])
        for (name, source) in [("main.tex", initialMain), ("second.tex", initialSecond), ("empty.tex", "")] {
            let file = project.appendingPathComponent(name)
            try Data(source.utf8).write(to: file, options: .withoutOverwriting)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
        }
        return (directory, configured != nil)
    }
}
extension StoreBodyCharacterCountWorkflowTests {
    static var initialMain: String { StoreWorkflowFixture.initialMain }
    static var initialSecond: String { StoreWorkflowFixture.initialSecond }
    @MainActor func append(_ text: String, to store: WorkspaceStore) throws {
        try store.edit(range: NSRange(location: store.text.utf16.count, length: 0), replacement: text)
        try storeWorkflowRequire(store.failure == nil, "Actual Store edit failed: \(store.failure ?? "")")
    }
    func disk(_ root: URL, _ name: String) throws -> Data {
        try Data(contentsOf: root.appendingPathComponent(name))
    }
    @MainActor func withStore(_ work: @MainActor (WorkspaceStore, URL, StoreWorkflowTrace) throws -> Void) throws {
        try storeWorkflowRequire(Thread.isMainThread, "Synchronous main-thread XCTest required")
        // All six methods are synchronous on MainActor. Limit the creation mask to this
        // own test process while its Runtime exists, and restore it after Stop/Join.
        // This provisions private socket creation; no existing endpoint is chmod-ed.
        StoreWorkflowFixture.creationMaskLock.lock()
        let previousMask = umask(0o077)
        defer {
            _ = umask(previousMask)
            StoreWorkflowFixture.creationMaskLock.unlock()
        }
        let fixture = try StoreWorkflowFixture.create()
        let directory = fixture.directory
        let root = directory.appendingPathComponent("project")
        if let compiler = Bundle.main.resourceURL?.appendingPathComponent("texlive/bin/universal-darwin/latexmk") {
            try storeWorkflowRequire(!FileManager.default.fileExists(atPath: compiler.path),
                                     "Compiler resource present: refusing Store.open")
        }
        let trace = StoreWorkflowTrace(directory, method: name)
        weak var weakStore: WorkspaceStore?
        var failure: Error?
        do {
            try autoreleasepool {
                let store = WorkspaceStore()
                weakStore = store
                store.open(root)
                try storeWorkflowRequire(store.failure == nil, "Runtime open failed: \(store.failure ?? "")")
                try storeWorkflowRequire(store.root?.resolvingSymlinksInPath().path == root.path, "Wrong Runtime root")
                try storeWorkflowRequire(store.selection == "main.tex" && store.revision == 1 && !store.dirty,
                                         "Initial genuine Runtime state mismatch")
                let binding = try StoreWorkflowSessionAuthority.admit(store.sessionPath, trace: trace)
                try storeWorkflowRequire(binding.normalizedParentMatches &&
                    binding.legacyPIDMatches && binding.legacyExtensionMatches,
                    "Foreign or malformed own Session path")
                try work(store, root, trace)
                try storeWorkflowRequire(store.failure == nil, "Unexpected final Store failure: \(store.failure ?? "")")
            }
        } catch { failure = error }
        trace.cleanup["store_deallocated"] = weakStore == nil
        if weakStore != nil && failure == nil {
            failure = StoreWorkflowError(description: "Store retained after scope; no foreign cleanup")
        }
        if let descriptor = trace.descriptor, weakStore == nil {
            do {
                let gone = try StoreWorkflowSessionAuthority.status(descriptor.path) == nil
                trace.cleanup["descriptor_removed_after_free"] = gone
                try storeWorkflowRequire(gone, "Own descriptor survived Runtime free")
            } catch { if failure == nil { failure = error } }
        }
        if weakStore == nil, let endpoint = trace.endpoint {
            do {
                if let socket = try StoreWorkflowSessionAuthority.status(endpoint.path) {
                    trace.cleanup["own_endpoint_residual_after_free"] = true
                    let parent = try StoreWorkflowSessionAuthority.noFollow(endpoint.deletingLastPathComponent().path)
                    try storeWorkflowRequire(parent.st_uid == geteuid() &&
                                (parent.st_mode & mode_t(S_IFMT)) == mode_t(S_IFDIR) &&
                                (parent.st_mode & 0o7777) == 0o700 &&
                                String(parent.st_dev) == trace.sessionCleanupIdentity["parent_dev"] &&
                                String(parent.st_ino) == trace.sessionCleanupIdentity["parent_inode"],
                                "Own cleanup parent authority drift")
                    try storeWorkflowRequire(socket.st_uid == geteuid() &&
                                (socket.st_mode & mode_t(S_IFMT)) == mode_t(S_IFSOCK) &&
                                (socket.st_mode & 0o077) == 0 &&
                                String(socket.st_dev) == trace.sessionCleanupIdentity["endpoint_dev"] &&
                                String(socket.st_ino) == trace.sessionCleanupIdentity["endpoint_inode"],
                                "Residual is not the same admitted own socket")
                    try FileManager.default.removeItem(at: endpoint)
                    trace.cleanup["own_residual_socket_removed_by_fixture"] = true
                }
                let gone = try StoreWorkflowSessionAuthority.status(endpoint.path) == nil
                trace.cleanup["own_endpoint_absent_after_cleanup"] = gone
                try storeWorkflowRequire(gone, "Own endpoint cleanup incomplete")
            } catch { if failure == nil { failure = error } }
        }

        trace.error = failure.map { String(describing: $0) }
        do {
            try trace.save()
        } catch let diagnosticError {
            if let failure {
                throw StoreWorkflowDiagnosticError(primary: failure, diagnostic: diagnosticError)
            }
            throw diagnosticError
        }
        if !fixture.retain && failure == nil {
            try FileManager.default.removeItem(at: directory)
        }
        if let failure { throw failure }
    }
    @MainActor func workflow(file: StaticString = #filePath, line: UInt = #line,
                             _ work: @MainActor (WorkspaceStore, URL, StoreWorkflowTrace) throws -> Void) {
        do { try withStore(work) }
        catch { XCTFail(String(describing: error), file: file, line: line) }
    }
}
