import SwiftUI
struct WorkspaceStatusBar: View {
    let store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    @State private var goToLine = false
    @State private var lineInput = ""
    var body: some View {
        HStack(spacing: 14) {
            Button { store.sidebar = .agent } label: { HStack(spacing: 5) { Image(systemName: "point.3.connected.trianglepath.dotted"); Circle().fill(theme.ui(store.sessionPath.isEmpty ? "secondary" : "success")).frame(width: 4, height: 4); Text("MCP") } }.buttonStyle(.plain).help("Helyi agentkapcsolat")
            Button { store.bottomPanel = .problems } label: {
                HStack(spacing: 4) { Image(systemName: "xmark.circle"); Text(String(store.errorCount)); Image(systemName: "exclamationmark.triangle").padding(.leading, 3); Text(String(store.warningCount)) }
            }.buttonStyle(.plain).help("Fordítási problémák")
            if store.completionIndexStatus?.truncated == true {
                Label("Részleges projektindex", systemImage: "exclamationmark.circle")
                    .lineLimit(1).foregroundStyle(theme.ui("warning"))
                    .help("A projektindex elérte a méretkorlátját, vagy néhány forrást nem tudott beolvasni. A jelöltek nem fedik le a teljes projektet.")
            }
            if store.building { ProgressView().controlSize(.mini); Text(store.buildLabel).lineLimit(1) }
            else if !store.buildState.isEmpty { Image(systemName: store.buildState == "succeeded" ? "checkmark" : "exclamationmark.circle").foregroundStyle(theme.ui(store.buildState == "succeeded" ? "success" : "warning")); Text(store.buildLabel).lineLimit(1) }
            if let notice = store.navigationNotice {
                Text(notice).lineLimit(1).truncationMode(.middle).frame(maxWidth: 280, alignment: .leading).layoutPriority(-1).help(notice)
            }
            Spacer(minLength: 8)
            if let selected = store.presentedDocuments.first(where: { $0.id == store.selection }), selected.origin == "agent" { Label("Agent", systemImage: "sparkle").foregroundStyle(theme.ui("accent")) }
            if store.selection != nil {
                Button { lineInput = String(store.caretLine); goToLine = true } label: { Text("Sor \(store.caretLine), oszlop \(store.caretColumn)").monospacedDigit() }.buttonStyle(.plain)
                    .popover(isPresented: $goToLine) {
                        HStack { Text("Ugrás a sorra"); TextField("Sor", text: $lineInput).frame(width: 70).onSubmit(jump); Button("Ugrás", action: jump).keyboardShortcut(.defaultAction) }.font(.system(size: 12)).padding(15)
                    }
                Text(store.bom ? "UTF-8 BOM" : "UTF-8")
                Text(store.lineEnding == "crlf" ? "CRLF" : "LF")
                Text("LaTeX")
            }
            if store.compilerReady { Text("XeLaTeX").foregroundStyle(theme.ui("environment")) }
        }.font(.system(size: 10.5)).foregroundStyle(theme.ui("secondary")).padding(.horizontal, 12).frame(height: 26).background(theme.ui("chrome"))
            .overlay(alignment: .top) { theme.ui("border").frame(height: 1) }
    }
    private func jump() {
        if let line = Int(lineInput), let file = store.selection { store.navigate(file: file, line: line); goToLine = false }
    }
}
