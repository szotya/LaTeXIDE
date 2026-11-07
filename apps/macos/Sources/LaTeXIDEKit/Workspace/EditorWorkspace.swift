import SwiftUI
struct EditorWorkspace: View {
    let store: WorkspaceStore
    let theme: ResolvedTheme
    let fontSize: Double
    var completionEnabled = true
    var body: some View {
        VStack(spacing: 0) {
            EditorTabs(store: store)
            HStack(spacing: 7) {
                Image(systemName: "folder").font(.system(size: 10))
                Text(store.root?.lastPathComponent ?? "Munkaterület").lineLimit(1)
                if let selection = store.selection {
                    ForEach(Array(selection.split(separator: "/").enumerated()), id: \.offset) { _, segment in
                        Image(systemName: "chevron.right").font(.system(size: 7, weight: .semibold))
                        Text(String(segment)).lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
                if store.isComposing { Text("Szövegbevitel…").foregroundStyle(theme.ui("accent")) }
                IDEIconButton(symbol: "magnifyingglass", label: "Keresés a fájlban · ⌘F", size: 23) { store.findInEditor() }.disabled(store.selection == nil)
                Menu {
                    Button("Összes mentése") { store.save() }
                    if let selection = store.selection {
                        Button("Megjelenítés a Finderben") { store.reveal(selection) }
                        if selection.hasSuffix(".tex") { Button("Beállítás fődokumentumnak") { store.mainFile = selection } }
                    }
                } label: { Image(systemName: "ellipsis").font(.system(size: 12)).foregroundStyle(theme.ui("secondary")) }.menuStyle(.borderlessButton).fixedSize().frame(width: 22)
            }.font(.system(size: 10.5)).foregroundStyle(theme.ui("secondary")).padding(.leading, 16).padding(.trailing, 8).frame(height: 30).background(theme.ui("editor"))
            Rectangle().fill(theme.ui("border").opacity(0.7)).frame(height: 1)
            if store.selection != nil { NativeEditor(store: store, theme: theme, fontSize: fontSize, completionEnabled: completionEnabled).clipped() }
            else { WorkspaceStart(store: store) }
        }.background(theme.ui("editor"))
    }
}
private struct EditorTabs: View {
    let store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    var body: some View {
        HStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 0) {
                        ForEach(store.presentedTabs) { tab in
                            HStack(spacing: 6) {
                                Button { store.select(tab.id) } label: {
                                    HStack(spacing: 7) { FileTypeIcon(path: tab.id); Text(tab.title).font(.system(size: 11.5, weight: tab.id == store.selection ? .medium : .regular)); if tab.dirty { Circle().fill(theme.ui("accent")).frame(width: 5, height: 5) } }
                                        .padding(.leading, 13).frame(height: 36).contentShape(Rectangle())
                                }.buttonStyle(.plain).foregroundStyle(theme.ui(tab.id == store.selection ? "text" : "secondary"))
                                Button { store.hideTab(tab.id) } label: { Image(systemName: "xmark").font(.system(size: 8, weight: .medium)).foregroundStyle(theme.ui("secondary")).frame(width: 22, height: 30) }.buttonStyle(.plain).disabled(store.isComposing).help("Fül elrejtése · a tartalom és az undo megmarad").accessibilityLabel("Fül elrejtése: \(tab.title)")
                            }.padding(.trailing, 5).background(theme.ui(tab.id == store.selection ? "editor" : "sidebar"))
                                .overlay(alignment: .bottom) { if tab.id == store.selection { theme.ui("accent").frame(height: 2) } }
                                .overlay(alignment: .trailing) { theme.ui("border").opacity(0.6).frame(width: 1) }.id(tab.id).help(tab.id)
                        }
                    }
                }.onChange(of: store.selection) { _, value in if let value { proxy.scrollTo(value) } }
            }
            if store.presentedTabs.isEmpty { Text("Szerkesztő").font(.system(size: 11)).foregroundStyle(theme.ui("secondary")).padding(.horizontal, 14) }
            Spacer(minLength: 0)
            IDEIconButton(symbol: "doc.badge.plus", label: "Gyors fájlmegnyitás · ⌘P", size: 28) { store.showQuickOpen() }.padding(.trailing, 6)
        }.frame(height: 36).background(theme.ui("sidebar")).overlay(alignment: .bottom) { theme.ui("border").opacity(0.6).frame(height: 1) }
    }
}
private struct WorkspaceStart: View {
    let store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            VStack(alignment: .leading, spacing: 9) {
                Text("λ").font(.system(size: 48, weight: .light, design: .serif)).foregroundStyle(theme.ui("accent"))
                Text("LaTeX IDE").font(.system(size: 25, weight: .semibold)).foregroundStyle(theme.ui("text"))
                Text(store.root == nil ? "Minden a dokumentumod körül." : "Válassz egy fájlt a munkához.").font(.system(size: 12)).foregroundStyle(theme.ui("secondary"))
            }
            VStack(spacing: 15) {
                startAction("Projektmappa megnyitása", key: "⌘O") { store.chooseProject() }
                startAction("Fájl keresése", key: "⌘P") { store.showQuickOpen() }
                startAction("Parancsok", key: "⇧⌘P") { store.showCommands() }
                startAction("Paletta és megjelenés", key: "") { store.showAppearance() }
            }
        }.frame(width: 320).frame(maxWidth: .infinity, maxHeight: .infinity).background(theme.ui("editor"))
    }
    private func startAction(_ label: String, key: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { HStack { Text(label); Spacer(); Text(key).font(.system(size: 11, design: .monospaced)).foregroundStyle(theme.ui("secondary")) }.font(.system(size: 12)).foregroundStyle(theme.ui("text")) }.buttonStyle(.plain)
    }
}
