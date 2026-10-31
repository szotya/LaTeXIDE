import AppKit
import SwiftUI

enum CodexExecutableLocator {
    static func locate(configuredPath: String = "", environment: [String: String] = ProcessInfo.processInfo.environment) -> URL? {
        let manager = FileManager.default
        var candidates: [String] = []
        if !configuredPath.isEmpty { candidates.append((configuredPath as NSString).expandingTildeInPath) }
        for directory in (environment["PATH"] ?? "").split(separator: ":") where directory.hasPrefix("/") {
            candidates.append(String(directory) + "/codex")
        }
        candidates += [
            "/opt/homebrew/bin/codex", "/usr/local/bin/codex",
            "/Applications/Codex.app/Contents/Resources/codex",
            "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex"
        ]
        return candidates.first(where: { manager.isExecutableFile(atPath: $0) }).map { URL(fileURLWithPath: $0) }
    }
}

@MainActor struct AssistantPanel: View {
    @ObservedObject var session: AssistantSession
    var context: AssistantContext
    @Environment(\.ideTheme) private var theme
    @AppStorage("ide.codexExecutablePath") private var savedExecutablePath = ""
    @State private var executablePath = ""
    @State private var agentSettingsExpanded = false
    @FocusState private var composerFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            if !session.isConnected { connectionControls }
            if let error = session.error {
                HStack(alignment: .top, spacing: 7) {
                    Image(systemName: "exclamationmark.circle.fill")
                    Text(error).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                }.font(.system(size: 11)).foregroundStyle(theme.ui("error")).padding(12)
                    .background(theme.ui("sidebar"))
            }
            transcript
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            composer
        }
        .background(theme.ui("panel"))
        .onAppear {
            if executablePath.isEmpty { executablePath = CodexExecutableLocator.locate(configuredPath: savedExecutablePath)?.path ?? savedExecutablePath }
            agentSettingsExpanded = !FileManager.default.isExecutableFile(atPath: executablePath) || session.error != nil
        }
        .onChange(of: session.projectGeneration) { _, _ in
            session.draftText = ""; session.draftSelection = nil; session.draftDiagnostics = nil
        }
        .onChange(of: session.error) { _, value in if value != nil { agentSettingsExpanded = true } }
    }

    private var header: some View {
        HStack(spacing: 7) {
            Circle().fill(theme.ui(session.state == .failed ? "error" : session.isConnected ? "success" : "secondary")).frame(width: 5, height: 5)
            Text(session.state.title).font(.system(size: 10, weight: .medium)).foregroundStyle(theme.ui("secondary"))
            Spacer(minLength: 4)
            IDEIconButton(symbol: "plus.bubble", label: "Új beszélgetés", size: 26) {
                session.newConversation(); session.draftSelection = nil; session.draftDiagnostics = nil
            }.disabled(session.state == .connecting)
            if session.isConnected || session.state == .connecting {
                IDEIconButton(symbol: "power", label: "Codex leválasztása", size: 26) { session.disconnect() }
            }
        }.padding(.leading, 12).padding(.trailing, 7).frame(height: 34).background(theme.ui("sidebar"))
    }

    private var connectionControls: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Codex CLI").font(.system(size: 12, weight: .semibold)).foregroundStyle(theme.ui("text"))
                Spacer(minLength: 0)
                Button(session.state == .failed ? "Újracsatlakozás" : "Csatlakozás") {
                    let path = (executablePath.trimmingCharacters(in: .whitespacesAndNewlines) as NSString).expandingTildeInPath
                    savedExecutablePath = path
                    session.connect(executable: URL(fileURLWithPath: path))
                }.controlSize(.small).disabled(!session.hasProject || executablePath.isEmpty || session.state == .connecting)
            }
            Text("Helyi agentfolyamat, a Codexben beállított modellszolgáltatással. A modellhívás hálózatot és az előfizetésedet vagy API-keretedet használhatja.")
                .font(.system(size: 10)).foregroundStyle(theme.ui("secondary")).fixedSize(horizontal: false, vertical: true)
            if !session.hasProject { Text("Előbb nyiss meg egy projektet.").font(.system(size: 10)).foregroundStyle(theme.ui("secondary")) }
            DisclosureGroup(isExpanded: $agentSettingsExpanded) {
                HStack(spacing: 5) {
                    TextField("Codex CLI végrehajtható fájl", text: $executablePath)
                        .textFieldStyle(.roundedBorder).font(.system(size: 10, design: .monospaced))
                        .accessibilityLabel("Codex CLI útvonala")
                        .disabled(session.state == .connecting)
                    Button { chooseExecutable() } label: { Image(systemName: "folder") }
                        .help("Codex CLI kiválasztása…").accessibilityLabel("Codex CLI kiválasztása")
                        .disabled(session.state == .connecting)
                }.padding(.top, 5)
            } label: {
                Text("Agent beállításai").font(.system(size: 10)).foregroundStyle(theme.ui("secondary"))
            }
        }.padding(12).background(theme.ui("sidebar"))
    }

    private var transcript: some View {
        ScrollViewReader { scroll in
            ScrollView {
                if session.messages.isEmpty {
                    VStack(alignment: .leading, spacing: 10) {
                        Image(systemName: "text.bubble").font(.system(size: 23, weight: .light)).foregroundStyle(theme.ui("accent"))
                        Text("Dolgozz a forráson közösen").font(.system(size: 13, weight: .medium)).foregroundStyle(theme.ui("text"))
                        Text("Kérj magyarázatot, javítást vagy új LaTeX-részletet. Az agent az IDE élő dokumentumait szerkeszti; a változtatások visszavonhatók. Mentés külön kérésre történik.")
                            .font(.system(size: 11)).foregroundStyle(theme.ui("secondary")).fixedSize(horizontal: false, vertical: true)
                        Text("A beszélgetés ebben az ablakban él. A Csatlakozás önmagában nem indít modellhívást.")
                            .font(.system(size: 10)).foregroundStyle(theme.ui("secondary")).fixedSize(horizontal: false, vertical: true)
                    }.padding(20).frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    LazyVStack(alignment: .leading, spacing: 14) {
                        if session.transcriptWasTrimmed {
                            Text("A korábbi üzenetek egy része a megjelenítési korlát miatt már nem látható.")
                                .font(.system(size: 10)).foregroundStyle(theme.ui("secondary"))
                        }
                        ForEach(session.messages) { message in messageRow(message) }
                        Color.clear.frame(height: 1).id("assistant-bottom")
                    }.padding(14)
                }
            }
            .onChange(of: session.messages) { _, _ in scroll.scrollTo("assistant-bottom", anchor: .bottom) }
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func messageRow(_ message: AssistantMessage) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 5) {
                Image(systemName: symbol(message.role))
                Text(label(message.role)).fontWeight(.medium)
                if message.isStreaming { ProgressView().controlSize(.mini).scaleEffect(0.7) }
            }.font(.system(size: 10)).foregroundStyle(theme.ui(message.role == .user ? "accent" : "secondary"))
            if !message.text.isEmpty {
                Text(message.text).font(.system(size: message.role == .activity ? 10 : 12, design: message.role == .activity ? .monospaced : .default))
                    .foregroundStyle(theme.ui(message.role == .activity || message.role == .notice ? "secondary" : "text"))
                    .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }.padding(message.role == .user ? 10 : 0)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(message.role == .user ? theme.ui("sidebar") : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: 5))
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 5) {
                contextChip(symbol: "folder", text: context.projectName)
                if let document = context.documentID { contextChip(symbol: "doc.text", text: "\((document as NSString).lastPathComponent) · r\(context.revision)") }
                Spacer(minLength: 0)
            }
            if session.draftSelection != nil || session.draftDiagnostics != nil {
                HStack(spacing: 5) {
                    if session.draftSelection != nil { attachmentChip("Kijelölés") { session.draftSelection = nil } }
                    if session.draftDiagnostics != nil { attachmentChip("Diagnosztikák") { session.draftDiagnostics = nil } }
                    Spacer(minLength: 0)
                }
            }
            TextEditor(text: $session.draftText).font(.system(size: 12)).foregroundStyle(theme.ui("text"))
                .scrollContentBackground(.hidden).padding(5).frame(minHeight: 62, maxHeight: 110)
                .background(theme.ui("editor")).clipShape(RoundedRectangle(cornerRadius: 5))
                .overlay(RoundedRectangle(cornerRadius: 5).stroke(theme.ui("border")))
                .focused($composerFocused).accessibilityLabel("Üzenet a Codexnek")
            HStack(spacing: 8) {
                Button {
                    session.draftSelection = assistantBounded(context.selectionText, bytes: 12 * 1024)
                } label: { Image(systemName: "text.badge.plus") }.help("Aktuális kijelölés csatolása").accessibilityLabel("Kijelölés csatolása")
                    .disabled(context.selectionText.isEmpty || session.draftSelection != nil || !session.canSend)
                Button {
                    session.draftDiagnostics = assistantBounded(context.diagnosticsText, bytes: 8 * 1024)
                } label: { Image(systemName: "exclamationmark.bubble") }.help("Aktuális fordítási diagnosztikák csatolása").accessibilityLabel("Diagnosztikák csatolása")
                    .disabled(context.diagnosticsText.isEmpty || session.draftDiagnostics != nil || !session.canSend)
                Spacer(minLength: 4)
                if session.canCancel {
                    Button { session.cancel() } label: { Label("Leállítás", systemImage: "stop.fill") }
                        .disabled(session.state == .stopping)
                } else {
                    Button { send() } label: { Label("Küldés", systemImage: "arrow.up") }
                        .keyboardShortcut(.return, modifiers: .command)
                        .disabled(!session.canSend || session.draftText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }.controlSize(.small)
            if let model = session.modelName {
                Text(model).font(.system(size: 9, design: .monospaced)).foregroundStyle(theme.ui("secondary")).lineLimit(1)
            }
        }.padding(12).background(theme.ui("sidebar"))
    }

    private func contextChip(symbol: String, text: String) -> some View {
        Label(text, systemImage: symbol).font(.system(size: 9)).lineLimit(1).truncationMode(.middle)
            .foregroundStyle(theme.ui("secondary")).padding(.horizontal, 6).padding(.vertical, 4)
            .background(theme.ui("panel")).clipShape(RoundedRectangle(cornerRadius: 4))
    }

    private func attachmentChip(_ title: String, remove: @escaping () -> Void) -> some View {
        Button(action: remove) { Label(title, systemImage: "xmark.circle.fill").font(.system(size: 9)) }
            .buttonStyle(.plain).foregroundStyle(theme.ui("accent")).help("Csatolmány eltávolítása")
    }

    private func send() {
        var attachedContext = context
        attachedContext.selectionText = session.draftSelection ?? ""
        attachedContext.diagnosticsText = session.draftDiagnostics ?? ""
        session.send(text: session.draftText, context: attachedContext.text(includeSelection: session.draftSelection != nil, includeDiagnostics: session.draftDiagnostics != nil))
        if session.state == .running { session.draftText = ""; session.draftSelection = nil; session.draftDiagnostics = nil; composerFocused = true }
    }

    private func chooseExecutable() {
        let picker = NSOpenPanel()
        picker.title = "Codex CLI kiválasztása"
        picker.prompt = "Kiválasztás"
        picker.canChooseDirectories = false
        picker.allowsMultipleSelection = false
        picker.showsHiddenFiles = true
        picker.treatsFilePackagesAsDirectories = true
        if !executablePath.isEmpty { picker.directoryURL = URL(fileURLWithPath: executablePath).deletingLastPathComponent() }
        if picker.runModal() == .OK, let url = picker.url { executablePath = url.path }
    }

    private func symbol(_ role: AssistantMessage.Role) -> String {
        switch role { case .user: "person.crop.circle"; case .assistant: "sparkle"; case .activity: "wrench.and.screwdriver"; case .notice: "info.circle" }
    }
    private func label(_ role: AssistantMessage.Role) -> String {
        switch role { case .user: "Te"; case .assistant: "Codex"; case .activity: "Eszköz"; case .notice: "Állapot" }
    }
}
