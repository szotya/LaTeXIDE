import SwiftUI
struct InspectorPanel: View {
    @ObservedObject var store: WorkspaceStore
    @ObservedObject var session: AssistantSession
    @Environment(\.ideTheme) private var theme
    private var context: AssistantContext {
        let source = store.text as NSString
        let fullRange = EditorViewState.safeSelection(store.editorSelection, in: store.text)
        let bounded = EditorViewState.safeSelection(NSRange(location: fullRange.location, length: min(fullRange.length, 8192)), in: store.text)
        let selected = bounded.length == 0 ? "" : source.substring(with: bounded)
        let diagnostics = store.diagnostics.prefix(20).map { "\($0.file ?? ""): \($0.line ?? 0): \($0.message)" }.joined(separator: "\n")
        return AssistantContext(projectName: store.root?.lastPathComponent ?? "Nincs projekt", documentID: store.selection,
            revision: store.revision, selectionText: selected, diagnosticsText: String(diagnostics.prefix(8000)))
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                tab("PDF", symbol: "doc.richtext", value: .pdf)
                tab("Asszisztens", symbol: "sparkles", value: .assistant)
                Spacer(minLength: 0)
                IDEIconButton(symbol: "xmark", label: "Jobb oldali panel elrejtése", size: 24) { store.pdfVisible = false }.padding(.trailing, 5)
            }.frame(height: 36).background(theme.ui("sidebar"))
            theme.ui("border").frame(height: 1)
            if store.inspector == .assistant { AssistantPanel(session: session, context: context) }
            else { PDFPanel(store: store, embedded: true) }
        }.background(theme.ui("panel"))
    }
    private func tab(_ title: String, symbol: String, value: InspectorTab) -> some View {
        Button { store.inspector = value } label: {
            Label(title, systemImage: symbol).font(.system(size: 11, weight: store.inspector == value ? .medium : .regular))
                .foregroundStyle(theme.ui(store.inspector == value ? "text" : "secondary")).padding(.horizontal, 13).frame(height: 36)
                .overlay(alignment: .bottom) { if store.inspector == value { theme.ui("accent").frame(height: 2) } }
        }.buttonStyle(.plain)
    }
}
