import AppKit
import SwiftUI

private final class CompletionPanelWindow: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

@MainActor final class CompletionController: ObservableObject {
    @Published private(set) var proposal: CompletionProposal?
    @Published private(set) var selectedIndex = 0
    private weak var coordinator: NativeEditor.Coordinator?
    private weak var source: SourceTextView?
    private var panel: NSPanel?
    private var proposalCaret = 0
    private var dismissedKey: String?
    init(coordinator: NativeEditor.Coordinator) { self.coordinator = coordinator }

    func refresh(_ view: SourceTextView, explicit: Bool = false) {
        guard let coordinator, coordinator.completionEnabled, !coordinator.rendering,
              !coordinator.store.isComposing, !view.hasMarkedText(), view.selectedRange().length == 0,
              view.string == coordinator.store.text else { dismiss(); return }
        if let window = view.window, window.firstResponder !== view { dismiss(); return }
        let caret = view.selectedRange().location
        let key = "\(coordinator.store.selection ?? ""):\(coordinator.store.revision):\(caret)"
        if !explicit && dismissedKey == key { return }
        guard let reply = coordinator.store.completions(caret: caret) else { dismiss(); return }
        guard reply.documentID == coordinator.store.selection, reply.revision == coordinator.store.revision else { dismiss(); return }
        proposalCaret = caret; source = view; proposal = reply; selectedIndex = 0
        show(view, theme: coordinator.theme)
    }
    func dismiss(remember: Bool = false) {
        if remember, let coordinator { dismissedKey = "\(coordinator.store.selection ?? ""):\(coordinator.store.revision):\(proposalCaret)" }
        if let parent = panel?.parent { parent.removeChildWindow(panel!) }
        panel?.orderOut(nil)
        // HostingView observes this controller; release it to break controller→panel→host→controller.
        panel?.contentView = nil; panel = nil
        proposal = nil
    }
    private func isCurrent(_ view: SourceTextView) -> Bool {
        guard let coordinator, let proposal else { return false }
        return proposal.documentID == coordinator.store.selection && proposal.revision == coordinator.store.revision
            && view.selectedRange() == NSRange(location: proposalCaret, length: 0)
            && view.string == coordinator.store.text && !view.hasMarkedText() && !coordinator.store.isComposing
    }
    @discardableResult func accept(_ view: SourceTextView) -> Bool {
        guard let coordinator, let proposal, proposal.items.indices.contains(selectedIndex),
              proposal.documentID == coordinator.store.selection, proposal.revision == coordinator.store.revision,
              view.selectedRange() == NSRange(location: proposalCaret, length: 0),
              view.string == coordinator.store.text, !view.hasMarkedText(), !coordinator.store.isComposing else { dismiss(); return false }
        let item = proposal.items[selectedIndex]
        // A definition can change in another buffer without changing this document's revision.
        guard let fresh = coordinator.store.completions(caret: proposalCaret, refreshProject: true), fresh.range == proposal.range,
              fresh.items.contains(where: { $0.label == item.label && $0.kind == item.kind && $0.insertText == item.insertText && $0.selectionStartUTF16 == item.selectionStartUTF16 && $0.selectionLengthUTF16 == item.selectionLengthUTF16 }) else { dismiss(); return false }
        let count = (view.string as NSString).length, inserted = (item.insertText as NSString).length
        guard proposal.range.location >= 0, proposal.range.length >= 0,
              proposal.range.location <= count - min(count, proposal.range.length), proposal.range.length <= count,
              item.selectionStartUTF16 >= 0, item.selectionLengthUTF16 >= 0,
              item.selectionStartUTF16 <= inserted - min(inserted, item.selectionLengthUTF16), item.selectionLengthUTF16 <= inserted,
              (try? SourceEdit.make(text: view.string, range: proposal.range, replacement: item.insertText)) != nil else { dismiss(); return false }
        let unchanged = (view.string as NSString).substring(with: proposal.range) == item.insertText
        dismiss()
        if !unchanged {
            view.insertText(item.insertText, replacementRange: proposal.range)
            guard coordinator.store.text == view.string, coordinator.store.revision != proposal.revision else { return false }
        }
        view.setSelectedRange(NSRange(location: proposal.range.location + item.selectionStartUTF16, length: item.selectionLengthUTF16))
        dismissedKey = "\(coordinator.store.selection ?? ""):\(coordinator.store.revision):\(view.selectedRange().location)"
        view.window?.makeFirstResponder(view)
        return true
    }
    func handle(_ selector: Selector, view: SourceTextView) -> Bool {
        guard let proposal, !view.hasMarkedText() else { return false }
        guard isCurrent(view) else { dismiss(); return false }
        switch NSStringFromSelector(selector) {
        case "moveDown:": selectedIndex = min(proposal.items.count - 1, selectedIndex + 1)
        case "moveUp:": selectedIndex = max(0, selectedIndex - 1)
        case "insertTab:", "insertNewline:": return accept(view)
        case "cancelOperation:": dismiss(remember: true)
        default: return false
        }
        return true
    }
    private func show(_ view: SourceTextView, theme: ResolvedTheme) {
        guard let parent = view.window, let proposal else { return }
        if panel == nil {
            let window = CompletionPanelWindow(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
            window.isOpaque = false; window.backgroundColor = .clear; window.hasShadow = true
            window.hidesOnDeactivate = true; window.isReleasedWhenClosed = false; window.level = .floating
            panel = window
        }
        guard let panel else { return }
        let height = CGFloat(min(proposal.items.count, 7)) * 39 + 30
        panel.contentView = NSHostingView(rootView: CompletionCandidates(controller: self, theme: theme))
        var caret = view.firstRect(forCharacterRange: view.selectedRange(), actualRange: nil)
        if caret.width == 0 { caret.size.width = 1 }
        let visible = parent.screen?.visibleFrame ?? parent.frame
        let width: CGFloat = min(440, visible.width - 24)
        var origin = NSPoint(x: caret.minX, y: caret.minY - height - 5)
        if origin.y < visible.minY { origin.y = caret.maxY + 5 }
        origin.x = max(visible.minX + 8, min(origin.x, visible.maxX - width - 8))
        panel.setFrame(NSRect(origin: origin, size: NSSize(width: width, height: height)), display: true)
        if panel.parent !== parent { panel.parent?.removeChildWindow(panel); parent.addChildWindow(panel, ordered: .above) }
        panel.orderFront(nil)
    }
    func choose(_ index: Int) { guard let source else { return }; selectedIndex = index; _ = accept(source) }
}

private struct CompletionCandidates: View {
    @ObservedObject var controller: CompletionController
    let theme: ResolvedTheme
    private var title: String {
        switch controller.proposal?.items.first?.kind {
        case "reference": "Címkék"
        case "citation": "BibTeX"
        case "macro": "Saját makrók"
        default: "LaTeX"
        }
    }
    private func badge(_ kind: String?) -> String {
        switch kind { case "macro": "ƒ"; case "reference": "ref"; case "citation": "bib"; default: "TᴇX" }
    }
    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(Array((controller.proposal?.items ?? []).enumerated()), id: \.offset) { index, item in
                            Button { controller.choose(index) } label: {
                                HStack(spacing: 10) {
                                    Text(badge(item.kind)).font(.system(size: 9, weight: .bold, design: .monospaced)).foregroundStyle(theme.ui("environment")).frame(width: 24)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(item.label).font(.system(size: 12, weight: .medium, design: .monospaced)).foregroundStyle(theme.ui(item.kind == "reference" || item.kind == "citation" ? "text" : "command"))
                                        Text(item.detail).font(.system(size: 10)).foregroundStyle(theme.ui("secondary")).lineLimit(1)
                                    }
                                    Spacer(minLength: 0)
                                    if index == controller.selectedIndex { Text("↵").foregroundStyle(theme.ui("secondary")) }
                                }.padding(.horizontal, 10).frame(height: 39).background(theme.ui(index == controller.selectedIndex ? "selection" : "sidebar")).contentShape(Rectangle())
                            }.buttonStyle(.plain).id(index)
                        }
                    }
                }.onChange(of: controller.selectedIndex) { _, value in proxy.scrollTo(value) }
            }
            HStack {
                Text(title)
                if controller.proposal?.indexStatus?.truncated == true { Image(systemName: "exclamationmark.circle").help("Részleges projektindex; a jelöltek nem fedik le a teljes projektet.") }
                Spacer(); Text("↑ ↓ választás   ⇥ ↵ beszúrás   esc bezárás")
            }.font(.system(size: 9)).foregroundStyle(theme.ui("secondary")).padding(.horizontal, 10).frame(height: 29).background(theme.ui("chrome"))
        }.background(theme.ui("sidebar")).clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(theme.ui("border"), lineWidth: 1)).accessibilityElement(children: .contain).accessibilityLabel("LaTeX-kiegészítés")
    }
}
