import SwiftUI
import AppKit
private struct IDEThemeKey: EnvironmentKey { static let defaultValue = ThemeCatalog.shared.palette("graphite").resolve(dark: true) }
extension EnvironmentValues {
    var ideTheme: ResolvedTheme { get { self[IDEThemeKey.self] } set { self[IDEThemeKey.self] = newValue } }
}
struct IDEIconButton: View {
    let symbol: String
    let label: String
    var selected = false
    var size: CGFloat = 28
    let action: () -> Void
    @Environment(\.ideTheme) private var theme
    @Environment(\.isEnabled) private var enabled
    @State private var hover = false
    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 12, weight: .medium))
                .foregroundStyle(theme.ui(selected ? "accent" : "secondary"))
                .frame(width: size, height: size)
                .background(RoundedRectangle(cornerRadius: 5).fill(selected ? theme.ui("accent").opacity(0.14) : hover ? theme.ui("hover") : .clear))
        }.buttonStyle(.plain).opacity(enabled ? 1 : 0.35).onHover { hover = $0 }.help(label).accessibilityLabel(label)
    }
}
struct FileTypeIcon: View {
    let path: String
    @Environment(\.ideTheme) private var theme
    var body: some View {
        let ext = (path as NSString).pathExtension.lowercased()
        Group {
            if ext == "tex" { Text("TᴇX").font(.system(size: 9, weight: .bold, design: .serif)).foregroundStyle(theme.ui("environment")) }
            else { Image(systemName: ext == "bib" ? "books.vertical" : ext == "pdf" ? "doc.richtext" : "doc.text").font(.system(size: 12)).foregroundStyle(theme.ui(ext == "bib" ? "literal" : "secondary")) }
        }.frame(width: 19, height: 18)
    }
}
struct WindowChromeBridge: NSViewRepresentable {
    let theme: ResolvedTheme
    let appearance: AppearanceMode
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async { [weak view] in
            guard let window = view?.window else { return }
            window.titleVisibility = .hidden; window.titlebarAppearsTransparent = true
            window.isMovableByWindowBackground = true
            window.backgroundColor = theme.ns("chrome")
            window.appearance = appearance == .system ? nil : NSAppearance(named: theme.dark ? .darkAqua : .aqua)
        }
    }
}
extension WorkspaceStore {
    public func undoCommand(in window: NSWindow? = nil) { routeHistoryCommand(redo: false, window: window) }
    public func redoCommand(in window: NSWindow? = nil) { routeHistoryCommand(redo: true, window: window) }
    private func routeHistoryCommand(redo: Bool, window: NSWindow?) {
        if let field = (window ?? NSApp.keyWindow)?.firstResponder as? NSTextView, !(field is SourceTextView) {
            if field.hasMarkedText() { field.unmarkText() }
            if redo { if field.undoManager?.canRedo == true { field.undoManager?.redo() } }
            else if field.undoManager?.canUndo == true { field.undoManager?.undo() }
            return
        }
        if redo { self.redo() } else { self.undo() }
    }
    var buildLabel: String {
        switch buildState {
        case "queued": "Fordítás előkészítése"
        case "running": "Fordítás folyamatban"
        case "succeeded": "Sikeres fordítás"
        case "failed": "Sikertelen fordítás"
        case "cancelled": "Fordítás megszakítva"
        case "timed_out": "Fordítási időkorlát"
        default: compilerReady ? "Fordításra kész" : "Helyi munkaterület"
        }
    }
    public func findInEditor() {
        quickOpen = nil
        NotificationCenter.default.post(name: .latexIDEFind, object: self)
    }
    func reveal(_ path: String) {
        guard let root else { return }
        NSWorkspace.shared.activateFileViewerSelecting([root.appendingPathComponent(path)])
    }
}

extension Notification.Name { static let latexIDEFind = Notification.Name("hu.latexide.find") }
