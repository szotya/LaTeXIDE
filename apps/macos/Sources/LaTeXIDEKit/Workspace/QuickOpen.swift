import SwiftUI
private struct IDECommand: Identifiable {
    let id: String, title: String, symbol: String, shortcut: String
    let action: () -> Void
}
struct QuickOpen: View {
    @ObservedObject var store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    @State private var query = ""
    @State private var index = 0
    @FocusState private var focused: Bool
    private var commandMode: Bool { store.quickOpen == .commands || query.hasPrefix(">") }
    private var search: String { query.hasPrefix(">") ? String(query.dropFirst()).trimmingCharacters(in: .whitespaces) : query }
    private var commands: [IDECommand] {
        var commands = [
            IDECommand(id: "open", title: "Projektmappa megnyitása…", symbol: "folder", shortcut: "⌘O", action: store.chooseProject),
            IDECommand(id: "theme", title: "Paletta és megjelenés", symbol: "paintpalette", shortcut: "", action: store.showAppearance),
            IDECommand(id: "project", title: "Projektpanel megjelenítése / elrejtése", symbol: "sidebar.left", shortcut: "⌘1", action: store.toggleSidebar),
            IDECommand(id: "pdf", title: "PDF-előnézet megjelenítése / elrejtése", symbol: "doc.richtext", shortcut: "", action: store.togglePDF),
            IDECommand(id: "output", title: "Fordítási kimenet megjelenítése / elrejtése", symbol: "rectangle.bottomthird.inset.filled", shortcut: "⌘J", action: store.toggleOutput)
        ]
        if store.root != nil {
            commands += [
                IDECommand(id: "save", title: "Összes dokumentum mentése", symbol: "square.and.arrow.down", shortcut: "⌘S", action: store.save),
                IDECommand(id: "build", title: "LaTeX fordítása", symbol: "play", shortcut: "⌘B", action: store.build),
                IDECommand(id: "agent", title: "MCP-kapcsolódási beállítás másolása", symbol: "link", shortcut: "", action: store.copyMCPConfiguration)
            ]
        }
        if store.selection != nil { commands.append(IDECommand(id: "find", title: "Keresés az aktuális fájlban", symbol: "magnifyingglass", shortcut: "⌘F", action: store.findInEditor)) }
        return commands.filter { search.isEmpty || $0.title.localizedCaseInsensitiveContains(search) }
    }
    private var files: [String] { Array(store.files.filter { search.isEmpty || $0.localizedCaseInsensitiveContains(search) }.prefix(100)) }
    private var count: Int { commandMode ? commands.count : files.count }
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: commandMode ? "chevron.right" : "magnifyingglass").foregroundStyle(theme.ui("accent"))
                TextField(commandMode ? "Írj be egy parancsot…" : "Fájl megnyitása név alapján…", text: $query)
                    .textFieldStyle(.plain).font(.system(size: 14)).focused($focused).onSubmit(choose)
                    .onKeyPress(.downArrow) { index = min(max(0, count - 1), index + 1); return .handled }
                    .onKeyPress(.upArrow) { index = max(0, index - 1); return .handled }
                Text("esc").font(.system(size: 10, design: .monospaced)).foregroundStyle(theme.ui("secondary")).padding(.horizontal, 5).padding(.vertical, 2).overlay(RoundedRectangle(cornerRadius: 3).stroke(theme.ui("border"), lineWidth: 1))
            }.padding(15)
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            HStack { Text(commandMode ? "PARANCSOK" : "FÁJLOK"); Spacer(); Text(String(count)) }.font(.system(size: 9, weight: .semibold)).tracking(0.8).foregroundStyle(theme.ui("secondary")).padding(.horizontal, 15).frame(height: 27)
            if count == 0 {
                Text(store.root == nil && !commandMode ? "Előbb nyiss meg egy projektmappát." : "Nincs egyező találat.").font(.system(size: 12)).foregroundStyle(theme.ui("secondary")).padding(25)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 1) {
                            if commandMode {
                                ForEach(Array(commands.enumerated()), id: \.element.id) { offset, command in
                                    Button { index = offset; choose() } label: {
                                        HStack(spacing: 10) { Image(systemName: command.symbol).frame(width: 20); Text(command.title); Spacer(); Text(command.shortcut).font(.system(size: 10, design: .monospaced)).foregroundStyle(theme.ui("secondary")) }.font(.system(size: 12)).padding(.horizontal, 12).frame(height: 36).foregroundStyle(theme.ui("text")).background(theme.ui(offset == index ? "selection" : "sidebar")).contentShape(Rectangle())
                                    }.buttonStyle(.plain).id(offset)
                                }
                            } else {
                                ForEach(Array(files.enumerated()), id: \.element) { offset, file in
                                    Button { index = offset; choose() } label: {
                                        HStack(spacing: 9) {
                                            FileTypeIcon(path: file)
                                            Text((file as NSString).lastPathComponent).font(.system(size: 12))
                                            Text((file as NSString).deletingLastPathComponent).font(.system(size: 10)).foregroundStyle(theme.ui("secondary")).lineLimit(1).truncationMode(.middle)
                                            Spacer()
                                            if let doc = store.retainedDocuments.first(where: { $0.id == file }) {
                                                Text(doc.dirty ? "módosítva" : store.tabs.contains(where: { $0.id == file }) ? "nyitva" : "elrejtve").font(.system(size: 9)).foregroundStyle(theme.ui("secondary"))
                                            }
                                        }.padding(.horizontal, 12).frame(height: 36).foregroundStyle(theme.ui("text")).background(theme.ui(offset == index ? "selection" : "sidebar")).contentShape(Rectangle())
                                    }.buttonStyle(.plain).id(offset)
                                }
                            }
                        }.padding(5)
                    }.frame(maxHeight: 320).onChange(of: index) { _, value in proxy.scrollTo(value) }
                }
            }
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            HStack { Text("↑ ↓ kiválasztás"); Text("↵ megnyitás"); Spacer(); Text(commandMode ? "Parancspaletta" : "> parancsok") }.font(.system(size: 9)).foregroundStyle(theme.ui("secondary")).padding(.horizontal, 13).frame(height: 28)
        }.foregroundStyle(theme.ui("text")).background(theme.ui("sidebar")).clipShape(RoundedRectangle(cornerRadius: 9))
            .overlay(RoundedRectangle(cornerRadius: 9).stroke(theme.ui("border"), lineWidth: 1))
            .shadow(color: .black.opacity(0.32), radius: 24, y: 12).frame(width: 590)
            .onAppear { focused = true }.onChange(of: query) { _, _ in index = 0 }
            .onExitCommand { store.quickOpen = nil; store.focusEditor() }
    }
    private func choose() {
        guard count > 0 else { return }
        let selected = min(index, count - 1)
        if commandMode { let action = commands[selected].action; store.quickOpen = nil; action() }
        else { let file = files[selected]; store.quickOpen = nil; store.select(file) }
    }
}
