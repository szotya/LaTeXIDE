import AppKit
import Combine

@MainActor public final class WorkspaceStore: ObservableObject {
    @Published var root: URL?
    @Published var files: [String] = []
    @Published var selection: String?
    @Published var text = ""
    @Published var revision: UInt64 = 0
    @Published var dirty = false
    @Published var canUndo = false
    @Published var canRedo = false
    @Published var message = "Nyiss meg egy helyi projektmappát."
    @Published var failure: String?
    @Published var mainFile = "main.tex"
    @Published var operation: String?
    @Published var buildState = ""
    @Published var buildLog = ""
    @Published var pdf: URL?
    @Published var pdfIsOld = false
    @Published var sessionPath = ""
    @Published var isComposing = false
    @Published var tabs: [EditorTab] = []
    @Published var retainedDocuments: [EditorTab] = []
    @Published var sidebar: SidebarTool? = .project
    @Published var bottomPanel: BottomTool? = .output
    @Published var pdfVisible = true
    @Published var inspector: InspectorTab = .pdf
    @Published var quickOpen: QuickOpenMode?
    @Published var appearancePresented = false
    @Published var caretLine = 1
    @Published var caretColumn = 1
    @Published var lineEnding = "lf"
    @Published var bom = false
    @Published var navigation: EditorNavigation?
    @Published var navigationNotice: String?
    @Published var diagnostics: [CompilerDiagnostic] = []
    @Published var compilerReady = false
    @Published var editorFocusVersion = 0
    @Published var editorSelection = NSRange(location: 0, length: 0)
    @Published var completionIndexStatus: CompletionIndexStatus?
    // Presentation survives the editor view being removed after the last tab is hidden.
    var editorStates: [String: EditorViewState] = [:]
    private var cachedProjectNodes: [ProjectNode] = []
    private var navigationSerial = 0
    private var consumedNavigationSerial = 0
    private var reportedBuildState = ""
    private var lastPollKey = ""
    private var runtime: NativeRuntime?
    private var timer: Timer?
    public init() {
        timer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.poll() }
        }
    }
    deinit { timer?.invalidate() }
    var hiddenTabCount: Int { retainedDocuments.filter { doc in !tabs.contains(where: { $0.id == doc.id }) }.count }
    var projectNodes: [ProjectNode] { cachedProjectNodes }
    var errorCount: Int { diagnostics.filter { $0.severity == "error" }.count }
    var warningCount: Int { diagnostics.filter { $0.severity == "warning" }.count }
    public func showQuickOpen() { quickOpen = .files }
    public func showCommands() { quickOpen = .commands }
    public func toggleSidebar() { sidebar = sidebar == nil ? .project : nil }
    public func adjacentTab(_ direction: Int) {
        guard !isComposing, !tabs.isEmpty else { return }
        let index = tabs.firstIndex(where: { $0.id == selection }) ?? 0
        select(tabs[(index + direction + tabs.count) % tabs.count].id)
    }
    public func focusEditor() { editorFocusVersion += 1 }
    public func togglePDF() {
        if inspector == .pdf { pdfVisible.toggle() } else { inspector = .pdf; pdfVisible = true }
    }
    public func showAssistant() { inspector = .assistant; pdfVisible = true }
    public func toggleOutput() { bottomPanel = bottomPanel == nil ? .output : nil }
    public func showAppearance() { appearancePresented = true }
    var building: Bool { ["queued", "running"].contains(buildState) }
    func perform(_ body: () throws -> Void) { do { try body() } catch { failure = error.localizedDescription } }
    public func chooseProject() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true; panel.canChooseFiles = false; panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url, confirmLeavingWorkspace() { open(url, discardCurrent: true) }
    }
    public func open(_ url: URL, discardCurrent: Bool = false) {
        guard !isComposing else { failure = "Előbb véglegesítsd a szövegbevitelt."; return }
        guard discardCurrent || !hasUnsavedChanges else { failure = "Projektváltás előtt mentsd a módosításokat, vagy használd a Megnyitás párbeszédablakát."; return }
        perform {
            let candidate = try NativeRuntime(root: url)
            var all: [String] = [], offset = 0
            while true {
                let page = try candidate.call("list_project_files", ["offset": offset])
                guard let entries = page["files"] as? [[String: Any]], let total = page["total"] as? Int else {
                    throw RuntimeFailure(message: "Érvénytelen fájllista a runtime-tól.")
                }
                let batch = entries.compactMap { $0["path"] as? String }
                guard batch.count == entries.count, !batch.isEmpty || offset >= total else {
                    throw RuntimeFailure(message: "Hiányos fájllista a runtime-tól.")
                }
                all += batch; offset += batch.count
                if offset >= total { break }
            }
            runtime = candidate; root = url; files = all; cachedProjectNodes = ProjectNode.tree(files: all); selection = nil; text = ""; revision = 0; completionIndexStatus = nil
            tabs = []; retainedDocuments = []; editorStates = [:]; diagnostics = []; navigation = nil; navigationNotice = nil; lastPollKey = ""; reportedBuildState = ""
            operation = nil; pdf = nil; buildState = ""; buildLog = ""; failure = nil
            if let compiler = Bundle.main.resourceURL?.appendingPathComponent("texlive/bin/universal-darwin/latexmk"), FileManager.default.fileExists(atPath: compiler.path) {
                _ = try candidate.call("configure_compiler", ["compiler_path": compiler.path,
                    "cache_directory": url.appendingPathComponent(".latexide/texlive-cache").path,
                    "output_directory": url.appendingPathComponent(".latexide/builds").path, "backend": "texlive"])
            }
            mainFile = all.contains("main.tex") ? "main.tex" : all.first(where: { $0.hasSuffix(".tex") }) ?? "main.tex"
            if all.contains(mainFile) { try load(mainFile) }
            try refreshInfo()
            message = "Helyi projekt · \(url.lastPathComponent)"
        }
    }
    func select(_ path: String) {
        guard !isComposing else { failure = "Előbb véglegesítsd a szövegbevitelt."; return }
        navigationNotice = nil
        perform { try load(path); editorFocusVersion += 1 }
    }
    private func load(_ id: String, expectedRevision: UInt64? = nil) throws {
        guard let runtime else { return }
        let snapshot = try runtime.read(id)
        if let expectedRevision, snapshot.revision != expectedRevision { throw RuntimeFailure(message: "A célfájl közben megváltozott. Kattints újra a definícióhoz.") }
        selection = id; text = snapshot.text; revision = snapshot.revision; dirty = snapshot.dirty
        lineEnding = snapshot.lineEnding; bom = snapshot.bom
        let tab = EditorTab(id: id, revision: snapshot.revision, dirty: snapshot.dirty)
        if let index = tabs.firstIndex(where: { $0.id == id }) { tabs[index] = tab } else { tabs.append(tab) }
        try refreshInfo(reloadSelection: false)
    }
    func refreshInfo(reloadSelection: Bool = true) throws {
        guard let runtime else { return }
        let info = try runtime.call("workspace_info")
        let undo = info["can_undo"] as? Bool ?? false, redo = info["can_redo"] as? Bool ?? false
        if canUndo != undo { canUndo = undo }; if canRedo != redo { canRedo = redo }
        let descriptor = info["session_descriptor"] as? String ?? ""
        if sessionPath != descriptor { sessionPath = descriptor }
        let ready = info["compiler_ready"] as? Bool ?? false
        if compilerReady != ready { compilerReady = ready }
        if let build = info["build"] as? [String: Any] {
            let id = build["id"] as? String
            if operation != id { operation = id }
            reportedBuildState = build["state"] as? String ?? ""
        }
        let docs = (info["open_documents"] as? [[String: Any]] ?? []).compactMap(EditorTab.init)
        if retainedDocuments != docs { retainedDocuments = docs }
        let visible = tabs.map { tab in docs.first(where: { $0.id == tab.id }) ?? tab }
        if tabs != visible { tabs = visible }
        if !isComposing, let selection, let doc = docs.first(where: { $0.id == selection }) {
            if reloadSelection && doc.revision != revision { try load(selection) }
            else if dirty != doc.dirty { dirty = doc.dirty }
        }
    }
    func hideTab(_ id: String) {
        guard !isComposing else { return }
        guard let index = tabs.firstIndex(where: { $0.id == id }) else { return }
        tabs.remove(at: index)
        if selection == id {
            if !tabs.isEmpty { select(tabs[min(index, tabs.count - 1)].id) }
            else { selection = nil; text = ""; revision = 0; dirty = false }
        }
    }
    func navigate(file: String, line: Int) {
        select(file)
        guard selection == file else { return }
        navigationSerial += 1
        navigation = EditorNavigation(serial: navigationSerial, document: file, line: max(1, line))
    }
    func takeNavigation(for document: String?) -> EditorNavigation? {
        guard let navigation, navigation.document == document, navigation.serial != consumedNavigationSerial else { return nil }
        // Presentation consumption outlives a disposable editor coordinator.
        // This is not a published mutation during an NSViewRepresentable update.
        consumedNavigationSerial = navigation.serial
        return navigation
    }
    @discardableResult func goToDefinition(at offset: Int) -> Bool {
        guard let runtime, let selection, !isComposing, offset >= 0 else { return false }
        do {
            let reply = try DefinitionResolver.resolve(runtime: runtime, source: selection, revision: revision, offset: offset)
            if let status = reply.indexStatus, completionIndexStatus != status { completionIndexStatus = status }
            guard let target = reply.target, let targetRevision = target.revision else {
                navigationNotice = reply.symbol == nil ? nil : "Nincs projektdefiníció: \(reply.symbol!)\(reply.indexStatus?.truncated == true ? " · részleges index" : "")"
                return false
            }
            try load(target.documentID, expectedRevision: targetRevision)
            navigationSerial += 1; editorFocusVersion += 1
            navigation = EditorNavigation(serial: navigationSerial, document: target.documentID, line: target.line, range: target.range, revision: targetRevision)
            var notice = "Definíció · \(target.documentID):\(target.line)"
            if reply.alternatives > 1 { notice += " · \(reply.alternatives) lehetséges definíció" }
            if reply.indexStatus?.truncated == true { notice += " · részleges index" }
            navigationNotice = notice
            return true
        } catch { navigationNotice = error.localizedDescription; return false }
    }
    func reportCaret(line: Int, column: Int) {
        if caretLine != line { caretLine = line }; if caretColumn != column { caretColumn = column }
    }
    func syntax(document: String, revision: UInt64, range: NSRange) -> [CodeSpan]? {
        guard let runtime, !isComposing else { return nil }
        do {
            let result = try runtime.call("get_syntax", ["document_id": document, "expected_revision": revision,
                "offset_utf16": range.location, "length_utf16": min(range.length, 32768)])
            guard result["document_id"] as? String == document,
                  (result["revision"] as? NSNumber)?.uint64Value == revision else { return nil }
            return (result["spans"] as? [[Int]] ?? []).compactMap { row in
                guard row.count == 3 else { return nil }; return CodeSpan(start: row[0], length: row[1], kind: row[2])
            }
        } catch { return nil } // Coloring races are presentation work, never a modal edit error.
    }
    func completions(caret: Int, refreshProject: Bool = false) -> CompletionProposal? {
        guard let runtime, let selection, !isComposing, caret >= 0 else { return nil }
        do {
            let result = try runtime.call("get_completions", ["document_id": selection, "expected_revision": revision, "caret_utf16": caret, "refresh_project": refreshProject])
            let reply = try JSONDecoder().decode(CompletionProposal.self, from: JSONSerialization.data(withJSONObject: result))
            if let status = reply.indexStatus, completionIndexStatus != status { completionIndexStatus = status }
            guard reply.documentID == selection, reply.revision == revision, !reply.items.isEmpty else { return nil }
            return reply
        } catch { return nil }
    }
    func edit(range: NSRange, replacement: String, composition: Bool = false) throws {
        guard let runtime, let selection else { return }
        let edit = try SourceEdit.make(text: text, range: range, replacement: replacement)
        let encoded = try JSONSerialization.jsonObject(with: JSONEncoder().encode(edit))
        let info: [String: Any]
        do {
            info = try runtime.call(composition ? "end_composition" : "apply_edits",
                ["document_id": selection, "expected_revision": revision, "edits": [encoded]])
        } catch {
            if composition { isComposing = false }
            throw error
        }
        text = (text as NSString).replacingCharacters(in: range, with: replacement).replacingOccurrences(of: "\r\n", with: "\n")
        revision = (info["revision"] as! NSNumber).uint64Value; dirty = info["dirty"] as! Bool
        if composition { isComposing = false }
        pdfIsOld = pdf != nil
        try refreshInfo()
    }
    func beginComposition() -> Bool {
        guard !isComposing, let runtime, let selection else { return isComposing }
        do { _ = try runtime.call("begin_composition", ["document_id": selection, "expected_revision": revision]); isComposing = true; return true }
        catch { failure = error.localizedDescription; return false }
    }
    public func save() {
        guard !isComposing else { return }
        perform {
            guard let runtime else { return }
            let documents = try runtime.call("workspace_info")["open_documents"] as! [[String: Any]]
            for document in documents where document["dirty"] as? Bool == true {
                _ = try runtime.call("save_document", ["document_id": document["id"]!, "expected_revision": document["revision"]!])
            }
            if let selection { try load(selection) }; try refreshInfo(); message = "Mentve"
        }
    }
    public func undo() { history("undo") }
    public func redo() { history("redo") }
    private func history(_ op: String) {
        guard !isComposing else { return }
        perform { guard let runtime else { return }; let changed = try runtime.call(op); try load(changed["id"] as! String); try refreshInfo() }
    }
    public func build() {
        guard !isComposing else { return }
        save()
        perform {
            guard let runtime else { return }
            operation = try runtime.call("request_build", ["main_file": mainFile])["id"] as? String
            buildState = "queued"; pdfIsOld = pdf != nil; bottomPanel = .output; lastPollKey = ""
        }
    }
    func cancel() { perform { if let runtime, let operation { _ = try runtime.call("cancel_build", ["id": operation]) } } }
    func copyMCPConfiguration() {
        guard let executable = Bundle.main.resourceURL?.appendingPathComponent("latexide-mcp") else { return }
        let config: [String: Any] = ["mcpServers": ["latexide": ["command": executable.path, "args": ["--session", sessionPath]]]]
        perform { let data = try JSONSerialization.data(withJSONObject: config, options: [.prettyPrinted, .sortedKeys]); NSPasteboard.general.clearContents(); NSPasteboard.general.setString(String(decoding: data, as: UTF8.self), forType: .string); message = "MCP-konfiguráció a vágólapon · csak ehhez a nyitott munkamenethez" }
    }
    private func poll() {
        guard let runtime else { return }
        do {
            try refreshInfo()
            if let operation {
                let key = operation + "|" + reportedBuildState + "|" + retainedDocuments.map { "\($0.id):\($0.revision):\($0.dirty)" }.joined(separator: "|")
                if key == lastPollKey && !building { return }
                lastPollKey = key
                let status = try runtime.call("get_operation", ["id": operation])
                let nextState = status["state"] as? String ?? "", nextLog = status["log"] as? String ?? ""
                if buildState != nextState { buildState = nextState }; if buildLog != nextLog { buildLog = nextLog }
                let nextDiagnostics = (status["diagnostics"] as? [[String: Any]] ?? []).enumerated().map { CompilerDiagnostic(index: $0.offset, data: $0.element) }
                if diagnostics != nextDiagnostics { diagnostics = nextDiagnostics }
                if buildState == "succeeded", let path = status["pdf_path"] as? String {
                    let next = URL(fileURLWithPath: path); if pdf != next { pdf = next }
                }
                let old = pdf != nil && (buildState != "succeeded" || status["stale"] as? Bool == true)
                if pdfIsOld != old { pdfIsOld = old }
            }
        } catch { failure = error.localizedDescription }
    }
    public func requestTermination() -> Bool {
        guard confirmLeavingWorkspace() else { return false }
        timer?.invalidate(); timer = nil; self.runtime = nil
        return true
    }
    private var hasUnsavedChanges: Bool {
        guard let runtime else { return false }
        guard let documents = (try? runtime.call("workspace_info")["open_documents"]) as? [[String: Any]] else { return true }
        return documents.contains(where: { $0["dirty"] as? Bool == true })
    }
    private func confirmLeavingWorkspace() -> Bool {
        if isComposing { failure = "Véglegesítsd az aktív szövegbevitelt bezárás előtt."; return false }
        if hasUnsavedChanges {
            let alert = NSAlert()
            alert.messageText = "Mentetlen módosítások"
            alert.informativeText = "Mented a projekt módosításait a jelenlegi munkamenet bezárása előtt?"
            alert.addButton(withTitle: "Mentés"); alert.addButton(withTitle: "Mégse"); alert.addButton(withTitle: "Elvetés")
            switch alert.runModal() {
            case .alertFirstButtonReturn:
                failure = nil; save()
                if failure != nil || hasUnsavedChanges { return false }
            case .alertThirdButtonReturn: break
            default: return false
            }
        }
        return true
    }
}
