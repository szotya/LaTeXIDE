import SwiftUI
struct ProjectPanel: View {
    let store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    @State private var query = ""
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(store.sidebar == .agent ? "AGENTEK" : store.sidebar == .files ? "FÁJLNEVEK KERESÉSE" : "PROJEKT").font(.system(size: 10, weight: .semibold)).tracking(1)
                Spacer()
                IDEIconButton(symbol: "folder.badge.plus", label: "Projektmappa megnyitása") { store.chooseProject() }
                IDEIconButton(symbol: "sidebar.left", label: "Panel elrejtése") { store.sidebar = nil }
            }.foregroundStyle(theme.ui("secondary")).padding(.leading, 14).padding(.trailing, 5).frame(height: 38)
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            if store.sidebar == .agent { agentContent }
            else if store.sidebar == .files { fileSearch }
            else { projectTree }
            Spacer(minLength: 0)
            if store.root != nil {
                HStack {
                    Text("\(store.files.count) fájl").font(.system(size: 10))
                    Spacer()
                    if store.hiddenTabCount > 0 { Button("\(store.hiddenTabCount) rejtett fül") { store.showQuickOpen() }.buttonStyle(.plain).font(.system(size: 10)).foregroundStyle(theme.ui("accent")) }
                }.foregroundStyle(theme.ui("secondary")).padding(.horizontal, 13).frame(height: 29)
            }
        }.background(theme.ui("sidebar"))
    }
    private var projectTree: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let root = store.root {
                HStack(spacing: 6) { Image(systemName: "chevron.down").font(.system(size: 8, weight: .bold)); Text(root.lastPathComponent).font(.system(size: 11, weight: .semibold)).lineLimit(1); Spacer() }
                    .foregroundStyle(theme.ui("text")).padding(.horizontal, 12).frame(height: 32)
                ScrollView {
                    VStack(alignment: .leading, spacing: 1) { ForEach(store.projectNodes) { ProjectNodeRow(node: $0, store: store, level: 0) } }.padding(.bottom, 12)
                }
            } else {
                VStack(alignment: .leading, spacing: 12) { Text("Nincs megnyitott projekt").font(.system(size: 12, weight: .medium)); Text("Nyiss meg egy helyi mappát a fájlok böngészéséhez.").font(.system(size: 11)).foregroundStyle(theme.ui("secondary")); Button("Mappa megnyitása…") { store.chooseProject() }.font(.system(size: 11)) }.foregroundStyle(theme.ui("text")).padding(16)
            }
        }
    }
    private var fileSearch: some View {
        VStack(spacing: 10) {
            HStack { Image(systemName: "magnifyingglass"); TextField("Szűrés fájlnév szerint…", text: $query).textFieldStyle(.plain) }.font(.system(size: 11)).foregroundStyle(theme.ui("secondary")).padding(8).background(RoundedRectangle(cornerRadius: 5).fill(theme.ui("editor"))).overlay(RoundedRectangle(cornerRadius: 5).stroke(theme.ui("border"), lineWidth: 1)).padding(.horizontal, 10).padding(.top, 10)
            ScrollView { LazyVStack(spacing: 1) { ForEach(store.files.filter { query.isEmpty || $0.localizedCaseInsensitiveContains(query) }, id: \.self) { file in
                Button { store.select(file) } label: { HStack(spacing: 6) { FileTypeIcon(path: file); Text(file).lineLimit(1).truncationMode(.middle); Spacer() }.font(.system(size: 11)).foregroundStyle(theme.ui("text")).padding(.horizontal, 12).frame(height: 27).contentShape(Rectangle()) }.buttonStyle(.plain)
            } } }
        }
    }
    private var agentContent: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 7) { Circle().fill(theme.ui(store.sessionPath.isEmpty ? "secondary" : "success")).frame(width: 6, height: 6); Text(store.sessionPath.isEmpty ? "Nyiss meg egy projektet" : "Helyi MCP elérhető").font(.system(size: 12, weight: .medium)) }
            Text("Csatlakoztasd az agentedet a megnyitott projekthez. Módosításait a szerkesztőben visszavonhatod.").font(.system(size: 12)).foregroundStyle(theme.ui("secondary")).lineSpacing(4)
            Button { store.copyMCPConfiguration() } label: { Label("Kapcsolódási beállítás másolása", systemImage: "doc.on.doc").font(.system(size: 11)) }.disabled(store.sessionPath.isEmpty)
            Divider().overlay(theme.ui("border"))
            Label("Közvetlen szerkesztés", systemImage: "pencil.line")
            Label("Külön mentés", systemImage: "square.and.arrow.down")
            Label("Közös visszavonás", systemImage: "arrow.uturn.backward")
            if let document = store.presentedDocuments.first(where: { $0.id == store.selection }), document.origin == "agent" {
                Text("Az aktív fájlt legutóbb agent módosította.").font(.system(size: 11)).foregroundStyle(theme.ui("accent")).padding(.top, 4)
            }
        }.font(.system(size: 11)).foregroundStyle(theme.ui("text")).padding(15).frame(maxWidth: .infinity, alignment: .leading)
    }
}
private struct ProjectNodeRow: View {
    let node: ProjectNode
    let store: WorkspaceStore
    let level: Int
    @Environment(\.ideTheme) private var theme
    @State private var expanded = true
    @State private var hover = false
    var body: some View {
        VStack(spacing: 0) {
            Button {
                if let file = node.file { store.select(file) } else { expanded.toggle() }
            } label: {
                HStack(spacing: 6) {
                    if node.children != nil { Image(systemName: expanded ? "chevron.down" : "chevron.right").font(.system(size: 8, weight: .semibold)).frame(width: 10); Image(systemName: "folder").font(.system(size: 12)).foregroundStyle(theme.ui("secondary")) }
                    else { Color.clear.frame(width: 10); FileTypeIcon(path: node.id) }
                    Text(node.name).font(.system(size: 11.5)).lineLimit(1).truncationMode(.middle)
                    Spacer(minLength: 4)
                    if store.presentedDocuments.contains(where: { $0.id == node.id && $0.dirty }) { Circle().fill(theme.ui("accent")).frame(width: 5, height: 5) }
                }.foregroundStyle(theme.ui("text")).padding(.leading, CGFloat(8 + level * 14)).padding(.trailing, 10).frame(height: 26)
                    .background(theme.ui(store.selection == node.file && node.file != nil ? "selection" : hover ? "hover" : "sidebar")).contentShape(Rectangle())
            }.buttonStyle(.plain).onHover { hover = $0 }.help(node.id)
                .contextMenu {
                    if let file = node.file {
                        Button("Megnyitás") { store.select(file) }
                        if file.hasSuffix(".tex") { Button("Beállítás fődokumentumnak") { store.mainFile = file } }
                    }
                    Button("Megjelenítés a Finderben") { store.reveal(node.id) }
                }
            if expanded, let children = node.children { ForEach(children) { ProjectNodeRow(node: $0, store: store, level: level + 1) } }
        }
    }
}
