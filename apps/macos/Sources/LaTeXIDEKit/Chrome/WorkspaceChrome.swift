import SwiftUI
struct WorkspaceChrome: View {
    @Bindable var store: WorkspaceStore
    @ObservedObject var preferences: EditorPreferences
    @Environment(\.ideTheme) private var theme
    var body: some View {
        HStack(spacing: 12) {
            TitlebarActionButton(id: "workspace-project", title: "Projektmappa megnyitása · ⌘O", action: { store.chooseProject() }) {
                HStack(spacing: 8) {
                    ZStack { RoundedRectangle(cornerRadius: 5).fill(theme.ui("accent").opacity(0.17)).frame(width: 25, height: 25); Text("λ").font(.system(size: 18, weight: .semibold, design: .serif)).foregroundStyle(theme.ui("accent")) }
                    Text(store.root?.lastPathComponent ?? "LaTeX IDE").font(.system(size: 12, weight: .semibold)).lineLimit(1).truncationMode(.middle)
                    Image(systemName: "chevron.down").font(.system(size: 8, weight: .semibold)).foregroundStyle(theme.ui("secondary"))
                }.foregroundStyle(theme.ui("text"))
            }.buttonStyle(.plain).help("Projektmappa megnyitása · ⌘O").frame(minWidth: 120, maxWidth: 220, alignment: .leading).accessibilityIdentifier("workspace-project")
            TitlebarDragRegion().frame(minWidth: 8, maxWidth: .infinity)
            TitlebarActionButton(id: "workspace-quick-open", title: "Fájl vagy parancs keresése…", action: { store.showQuickOpen() }) {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass").font(.system(size: 11))
                    Text("Fájl vagy parancs keresése…").font(.system(size: 11)).lineLimit(1).truncationMode(.tail)
                    Spacer(minLength: 14)
                    Text("⌘P").font(.system(size: 10, weight: .medium, design: .monospaced))
                }.foregroundStyle(theme.ui("secondary")).padding(.horizontal, 10).frame(height: 28)
                    .background(RoundedRectangle(cornerRadius: 6).fill(theme.ui("editor")))
                    .overlay(RoundedRectangle(cornerRadius: 6).stroke(theme.ui("border"), lineWidth: 1))
            }.buttonStyle(.plain).frame(minWidth: 150, maxWidth: 360).layoutPriority(1).accessibilityIdentifier("workspace-quick-open")
            TitlebarDragRegion().frame(minWidth: 8, maxWidth: .infinity)
            TitlebarMainDocumentPicker(files: store.files.filter { $0.hasSuffix(".tex") }, selection: $store.mainFile)
                .frame(minWidth: 80, maxWidth: 155).disabled(store.root == nil).help("Fordítás fődokumentuma")
            TitlebarActionButton(id: "workspace-build", title: store.building ? "Leállítás" : "Fordítás", action: { store.building ? store.cancel() : store.build() }, usesIntrinsicWidth: true) {
                HStack(spacing: 6) { Image(systemName: store.building ? "stop.fill" : "play.fill").font(.system(size: 10)); Text(store.building ? "Leállítás" : "Fordítás").font(.system(size: 11, weight: .semibold)) }
                    .foregroundStyle(theme.ui(store.building ? "warning" : "success")).padding(.horizontal, 12).frame(height: 28)
                    .background(RoundedRectangle(cornerRadius: 6).fill(theme.ui(store.building ? "warning" : "success").opacity(0.12)))
            }.buttonStyle(.plain).fixedSize(horizontal: true, vertical: false).disabled(store.root == nil || store.isComposing).help("Fordítás · ⌘B").accessibilityIdentifier("workspace-build")
            Rectangle().fill(theme.ui("border")).frame(width: 1, height: 20)
            TitlebarIconButton(id: "workspace-sidebar", symbol: "sidebar.left", label: "Projektpanel · ⌘1", selected: store.sidebar != nil) { store.toggleSidebar() }.accessibilityIdentifier("workspace-sidebar")
            TitlebarIconButton(id: "workspace-pdf", symbol: "rectangle.trailinghalf.inset.filled", label: "PDF-előnézet", selected: store.pdfVisible && store.inspector == .pdf) { store.togglePDF() }.accessibilityIdentifier("workspace-pdf")
            TitlebarIconButton(id: "workspace-assistant", symbol: "sparkles", label: "Asszisztens · Codex", selected: store.pdfVisible && store.inspector == .assistant) { store.showAssistant() }.accessibilityIdentifier("workspace-assistant")
            TitlebarIconButton(id: "workspace-appearance", symbol: "circle.lefthalf.filled", label: "Paletta és megjelenés") { store.showAppearance() }.accessibilityIdentifier("workspace-appearance")
                .popover(isPresented: $store.appearancePresented, arrowEdge: .bottom) { AppearancePicker(preferences: preferences).environment(\.ideTheme, theme) }
        }.frame(height: 28)
    }
}
struct ActivityRail: View {
    @Bindable var store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    var body: some View {
        VStack(spacing: 8) {
            tool(.project, symbol: "square.stack.3d.up", label: "Projekt · ⌘1")
            tool(.files, symbol: "magnifyingglass", label: "Fájlnevek keresése")
            IDEIconButton(symbol: "sparkles", label: "Asszisztens · Codex", selected: store.pdfVisible && store.inspector == .assistant, size: 34) { store.showAssistant() }
            Spacer()
            IDEIconButton(symbol: "rectangle.bottomthird.inset.filled", label: "Alsó panel", selected: store.bottomPanel != nil, size: 34) { store.toggleOutput() }
            IDEIconButton(symbol: "paintpalette", label: "Megjelenés", size: 34) { store.showAppearance() }
        }.padding(.vertical, 10).frame(width: 46).background(theme.ui("chrome"))
    }
    private func tool(_ value: SidebarTool, symbol: String, label: String) -> some View {
        IDEIconButton(symbol: symbol, label: label, selected: store.sidebar == value, size: 34) { store.sidebar = store.sidebar == value ? nil : value }
            .overlay(alignment: .leading) { if store.sidebar == value { RoundedRectangle(cornerRadius: 1).fill(theme.ui("accent")).frame(width: 2, height: 17).offset(x: -6) } }
    }
}
