import SwiftUI
import PDFKit
@MainActor final class PDFControls: ObservableObject {
    weak var view: PDFView?
    @Published var page = 0
    @Published var count = 0
    @Published var scale = 1.0
    func update() {
        guard let view else { return }
        let nextCount = view.document?.pageCount ?? 0
        let index = view.currentPage.flatMap { view.document?.index(for: $0) }
        let nextPage = index.flatMap { $0 == NSNotFound ? nil : $0 + 1 } ?? 0
        if count != nextCount { count = nextCount }; if page != nextPage { page = nextPage }
        if scale != view.scaleFactor { scale = view.scaleFactor }
    }
    func previous() { view?.goToPreviousPage(nil) }
    func next() { view?.goToNextPage(nil) }
    func zoom(_ multiplier: Double) { guard let view else { return }; view.autoScales = false; view.scaleFactor = min(4, max(0.2, view.scaleFactor * multiplier)); update() }
    func fit() { view?.autoScales = true; update() }
}
private struct NativePDF: NSViewRepresentable {
    let url: URL?
    let theme: ResolvedTheme
    @ObservedObject var controls: PDFControls
    func makeCoordinator() -> Coordinator { Coordinator(controls) }
    func makeNSView(context: Context) -> PDFView {
        let view = PDFView(); view.autoScales = true; view.displayMode = .singlePageContinuous; view.displaysPageBreaks = true
        view.backgroundColor = theme.ns("panel"); view.setAccessibilityLabel("Fordított PDF")
        controls.view = view; context.coordinator.observe(view)
        return view
    }
    func updateNSView(_ view: PDFView, context: Context) {
        if view.document?.documentURL != url { view.document = url.flatMap { PDFDocument(url: $0) }; view.autoScales = true }
        view.backgroundColor = theme.ns("panel")
        DispatchQueue.main.async { controls.update() }
    }
    @MainActor final class Coordinator {
        let controls: PDFControls
        var tokens: [NSObjectProtocol] = []
        init(_ controls: PDFControls) { self.controls = controls }
        deinit { for token in tokens { NotificationCenter.default.removeObserver(token) } }
        func observe(_ view: PDFView) {
            for name in [Notification.Name.PDFViewPageChanged, .PDFViewScaleChanged, .PDFViewDocumentChanged] {
                tokens.append(NotificationCenter.default.addObserver(forName: name, object: view, queue: .main) { [weak controls] _ in DispatchQueue.main.async { controls?.update() } })
            }
        }
    }
}
struct PDFPanel: View {
    let store: WorkspaceStore
    var embedded = false
    @StateObject private var controls = PDFControls()
    @Environment(\.ideTheme) private var theme
    var body: some View {
        VStack(spacing: 0) {
            if !embedded { HStack(spacing: 8) {
                Image(systemName: "doc.richtext").foregroundStyle(theme.ui("error"))
                Text("PDF-előnézet").fontWeight(.medium)
                Spacer(minLength: 0)
                if store.pdfIsOld { Text("Korábbi eredmény").foregroundStyle(theme.ui("warning")).font(.system(size: 10)) }
                IDEIconButton(symbol: "xmark", label: "PDF-panel elrejtése", size: 24) { store.togglePDF() }
            }.font(.system(size: 11.5)).foregroundStyle(theme.ui("text")).padding(.leading, 13).padding(.trailing, 7).frame(height: 36).background(theme.ui("sidebar")) }
            HStack(spacing: 2) {
                IDEIconButton(symbol: "chevron.left", label: "Előző PDF-oldal", size: 24) { controls.previous() }.disabled(controls.page <= 1)
                Text(controls.count == 0 ? "– / –" : "\(controls.page) / \(controls.count)").font(.system(size: 10, design: .monospaced)).frame(minWidth: 40)
                IDEIconButton(symbol: "chevron.right", label: "Következő PDF-oldal", size: 24) { controls.next() }.disabled(controls.page >= controls.count)
                if embedded && store.pdfIsOld { Image(systemName: "clock").foregroundStyle(theme.ui("warning")).help("Korábbi fordítás eredménye") }
                Spacer()
                IDEIconButton(symbol: "minus", label: "PDF kicsinyítése", size: 24) { controls.zoom(1 / 1.15) }.disabled(store.pdf == nil)
                Text("\(Int(controls.scale * 100))%").font(.system(size: 10, design: .monospaced)).frame(width: 36)
                IDEIconButton(symbol: "plus", label: "PDF nagyítása", size: 24) { controls.zoom(1.15) }.disabled(store.pdf == nil)
                IDEIconButton(symbol: "arrow.up.left.and.arrow.down.right", label: "PDF ablakhoz igazítása", size: 24) { controls.fit() }.disabled(store.pdf == nil)
            }.foregroundStyle(theme.ui("secondary")).padding(.horizontal, 8).frame(height: 30).background(theme.ui("editor"))
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            if store.pdf != nil { NativePDF(url: store.pdf, theme: theme, controls: controls) }
            else {
                VStack(spacing: 13) {
                    Image(systemName: "doc.richtext").font(.system(size: 32, weight: .ultraLight)).foregroundStyle(theme.ui("secondary"))
                    Text("A dokumentum előnézete").font(.system(size: 13, weight: .medium)).foregroundStyle(theme.ui("text"))
                    Text("Fordítsd le a forrást az előnézethez.").font(.system(size: 11)).foregroundStyle(theme.ui("secondary"))
                    Button { store.build() } label: { Label("Fordítás", systemImage: "play.fill").font(.system(size: 11)) }.disabled(store.root == nil || store.building || store.isComposing)
                }.frame(maxWidth: .infinity, maxHeight: .infinity).background(theme.ui("panel"))
            }
        }
    }
}
