import AppKit
import SwiftUI

@MainActor struct NativeEditor: NSViewRepresentable {
    @ObservedObject var store: WorkspaceStore
    var theme: ResolvedTheme = ThemeCatalog.shared.palette("graphite").resolve(dark: true)
    var fontSize: Double = 14
    var completionEnabled = true
    func makeCoordinator() -> Coordinator { Coordinator(store) }
    func makeNSView(context: Context) -> NSScrollView {
        makeScrollView(coordinator: context.coordinator)
    }
    func makeScrollView(coordinator: Coordinator) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true; scroll.hasHorizontalScroller = true; scroll.borderType = .noBorder
        scroll.autohidesScrollers = true; scroll.scrollerStyle = .overlay
        let view = SourceTextView(frame: NSRect(x: 0, y: 0, width: 700, height: 600))
        view.adapter = coordinator; view.delegate = coordinator
        view.isRichText = false; view.importsGraphics = false; view.allowsUndo = false
        view.isAutomaticQuoteSubstitutionEnabled = false; view.isAutomaticDashSubstitutionEnabled = false
        view.isAutomaticTextReplacementEnabled = false; view.isAutomaticSpellingCorrectionEnabled = false
        view.isContinuousSpellCheckingEnabled = false; view.isGrammarCheckingEnabled = false
        view.usesFindBar = true; view.isIncrementalSearchingEnabled = true
        view.font = .monospacedSystemFont(ofSize: fontSize, weight: .regular)
        view.textContainerInset = NSSize(width: 14, height: 18)
        view.isVerticallyResizable = true; view.isHorizontallyResizable = true
        view.minSize = .zero; view.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.autoresizingMask = [.width]; view.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.textContainer?.widthTracksTextView = false
        view.setAccessibilityLabel("LaTeX-forrás")
        scroll.documentView = view
        scroll.verticalRulerView = LineNumberRuler(scrollView: scroll, source: view)
        scroll.hasVerticalRuler = true; scroll.rulersVisible = true
        scroll.contentView.postsBoundsChangedNotifications = true
        coordinator.observe(scroll: scroll, view: view)
        return scroll
    }
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        synchronize(scroll, coordinator: context.coordinator)
    }
    static func dismantleNSView(_ scroll: NSScrollView, coordinator: Coordinator) {
        if let view = scroll.documentView as? SourceTextView { coordinator.capturePresentation(view) }
        coordinator.completion.dismiss()
    }
    func synchronize(_ scroll: NSScrollView, coordinator: Coordinator) {
        guard let view = scroll.documentView as? SourceTextView else { return }
        coordinator.store = store; coordinator.theme = theme; coordinator.fontSize = fontSize
        coordinator.completionEnabled = completionEnabled
        if !completionEnabled { coordinator.completion.dismiss() }
        guard !store.isComposing else { return }
        let documentChanged = coordinator.document != store.selection || coordinator.project != store.root
        let changed = documentChanged || coordinator.renderedRevision != store.revision
        if changed {
            coordinator.completion.dismiss()
            if documentChanged { coordinator.capturePresentation(view) }
            let state = documentChanged
                ? store.editorStates[store.selection ?? ""] ?? EditorViewState(selection: NSRange(location: 0, length: 0), scrollOrigin: .zero)
                : EditorViewState(selection: view.selectedRange(), scrollOrigin: scroll.contentView.bounds.origin)
            coordinator.rendering = true
            if view.string != store.text { view.string = store.text }
            view.setSelectedRange(EditorViewState.safeSelection(state.selection, in: view.string))
            coordinator.document = store.selection; coordinator.project = store.root
            coordinator.renderedRevision = store.revision; coordinator.lineMap = SourceLineMap(view.string)
            if let container = view.textContainer {
                view.layoutManager?.ensureLayout(forBoundingRect: NSRect(origin: state.scrollOrigin, size: scroll.contentSize), in: container)
            }
            scroll.contentView.scroll(to: state.scrollOrigin); scroll.reflectScrolledClipView(scroll.contentView)
            coordinator.rendering = false
            let expectedDocument = store.selection, expectedRevision = store.revision
            DispatchQueue.main.async { [weak view, weak coordinator] in
                guard let view, let coordinator, coordinator.store.selection == expectedDocument, coordinator.store.revision == expectedRevision else { return }
                coordinator.reportCaret(view)
            }
        }
        if let navigation = store.takeNavigation(for: store.selection) {
            if navigation.revision == nil || navigation.revision == store.revision {
                let selection = navigation.range ?? NSRange(location: coordinator.lineMap.offset(line: navigation.line), length: 0)
                view.setSelectedRange(EditorViewState.safeSelection(selection, in: view.string))
                view.scrollRangeToVisible(view.selectedRange()); view.window?.makeFirstResponder(view)
            }
        }
        if coordinator.lastFocus != store.editorFocusVersion {
            coordinator.lastFocus = store.editorFocusVersion; view.window?.makeFirstResponder(view)
        }
        view.isEditable = store.selection != nil
        if changed || view.editorTheme != theme || view.font?.pointSize != CGFloat(fontSize) { coordinator.applyColoring(view, theme: theme) }
    }
    @MainActor final class Coordinator: NSObject, NSTextViewDelegate {
        var store: WorkspaceStore
        var rendering = false
        var document: String?
        var project: URL?
        var renderedRevision: UInt64 = 0
        var lineMap = SourceLineMap()
        var theme = ThemeCatalog.shared.palette("graphite").resolve(dark: true)
        var fontSize: Double = 14
        var completionEnabled = true
        lazy var completion = CompletionController(coordinator: self)
        var lastFocus = 0
        private var pending: (NSRange, String)?
        private var work: DispatchWorkItem?
        private var completionWork: DispatchWorkItem?
        private var observer: NSObjectProtocol?
        private var findObserver: NSObjectProtocol?
        init(_ store: WorkspaceStore) { self.store = store; self.project = store.root }
        deinit { work?.cancel(); completionWork?.cancel(); if let observer { NotificationCenter.default.removeObserver(observer) }; if let findObserver { NotificationCenter.default.removeObserver(findObserver) } }
        func observe(scroll: NSScrollView, view: SourceTextView) {
            findObserver = NotificationCenter.default.addObserver(forName: .latexIDEFind, object: store, queue: .main) { [weak view] _ in
                guard let view else { return }; view.window?.makeFirstResponder(view)
                let item = NSMenuItem(); item.tag = NSTextFinder.Action.showFindInterface.rawValue
                view.performTextFinderAction(item)
            }
            observer = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: scroll.contentView, queue: .main) { [weak self, weak view] _ in
                guard let self, let view else { return }; self.schedule(view)
                self.completion.dismiss()
                self.capturePresentation(view)
                scroll.verticalRulerView?.needsDisplay = true
            }
        }
        func capturePresentation(_ view: SourceTextView) {
            guard !rendering, let document, project == store.root else { return }
            store.editorStates[document] = EditorViewState(selection: view.selectedRange(), scrollOrigin: view.enclosingScrollView?.contentView.bounds.origin ?? .zero)
        }
        func schedule(_ view: SourceTextView, immediate: Bool = false) {
            work?.cancel()
            let item = DispatchWorkItem { [weak self, weak view] in guard let self, let view else { return }; self.applyColoring(view, theme: self.theme) }
            work = item
            DispatchQueue.main.asyncAfter(deadline: .now() + (immediate ? 0 : 0.10), execute: item)
        }
        func scheduleCompletion(_ view: SourceTextView) {
            completionWork?.cancel()
            let item = DispatchWorkItem { [weak self, weak view] in guard let self, let view else { return }; self.completion.refresh(view) }
            completionWork = item
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.08, execute: item)
        }
        func applyColoring(_ view: SourceTextView, theme: ResolvedTheme) {
            guard !store.isComposing, !view.hasMarkedText(), let id = store.selection, view.string == store.text else { return }
            let count = (view.string as NSString).length
            var range = NSRange(location: 0, length: min(count, 32768))
            if let scroll = view.enclosingScrollView, let manager = view.layoutManager, let container = view.textContainer, count > 0 {
                let visible = scroll.documentVisibleRect.offsetBy(dx: -view.textContainerOrigin.x, dy: -view.textContainerOrigin.y)
                manager.ensureLayout(forBoundingRect: visible, in: container)
                let glyphs = manager.glyphRange(forBoundingRectWithoutAdditionalLayout: visible, in: container)
                if glyphs.location != NSNotFound, glyphs.length > 0 {
                    let characters = manager.characterRange(forGlyphRange: glyphs, actualGlyphRange: nil)
                    let start = min(characters.location, count)
                    range = NSRange(location: start, length: min(32768, max(1, characters.length + 512), count - start))
                }
            }
            let revision = store.revision
            guard let spans = store.syntax(document: id, revision: revision, range: range), store.selection == id, store.revision == revision else { return }
            let selected = view.selectedRange(); let origin = view.enclosingScrollView?.contentView.bounds.origin
            rendering = true
            if view.editorTheme != theme || view.font?.pointSize != CGFloat(fontSize) {
                view.editorTheme = theme; view.backgroundColor = theme.ns("editor"); view.textColor = theme.ns("text")
                view.insertionPointColor = theme.ns("text"); view.selectedTextAttributes = [.backgroundColor: theme.ns("selection"), .foregroundColor: theme.ns("text")]
                view.font = .monospacedSystemFont(ofSize: fontSize, weight: .regular)
                let paragraph = NSMutableParagraphStyle(); paragraph.lineHeightMultiple = 1.40; paragraph.defaultTabInterval = fontSize * 2.4
                view.defaultParagraphStyle = paragraph
                if count > 0 { view.textStorage?.addAttribute(.paragraphStyle, value: paragraph, range: NSRange(location: 0, length: count)) }
            }
            view.textStorage?.beginEditing()
            if range.length > 0 { view.textStorage?.addAttribute(.foregroundColor, value: theme.ns("text"), range: range) }
            for span in spans where span.start >= 0 && span.length > 0 && span.start <= count - span.length {
                view.textStorage?.addAttribute(.foregroundColor, value: theme.syntaxColor(kind: span.kind), range: NSRange(location: span.start, length: span.length))
            }
            view.textStorage?.endEditing()
            view.typingAttributes = [.font: NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular), .foregroundColor: theme.ns("text"), .paragraphStyle: view.defaultParagraphStyle ?? NSParagraphStyle.default]
            view.setSelectedRange(selected)
            if let origin { view.enclosingScrollView?.contentView.scroll(to: origin) }
            rendering = false
            view.setNeedsDisplay(view.visibleRect, avoidAdditionalLayout: true)
            view.enclosingScrollView?.verticalRulerView?.needsDisplay = true
        }
        func beginComposition() -> Bool { pending = nil; work?.cancel(); completionWork?.cancel(); completion.dismiss(); return store.beginComposition() }
        /// Align accepted normalized text and its UTF-16 selection without a new edit.
        private func synchronizeNormalizedText(_ view: NSTextView) {
            let raw = view.string
            // Canonically equal strings can still have different UTF-16 offsets.
            guard (raw as NSString).compare(store.text, options: .literal) != .orderedSame else { return }
            let selected = EditorViewState.safeSelection(view.selectedRange(), in: raw)
            let source = raw as NSString
            func normalizedOffset(_ offset: Int) -> Int {
                source.substring(to: offset).replacingOccurrences(of: "\r\n", with: "\n").utf16.count
            }
            let start = normalizedOffset(selected.location), end = normalizedOffset(NSMaxRange(selected))
            rendering = true
            defer { rendering = false }
            view.string = store.text
            view.setSelectedRange(EditorViewState.safeSelection(NSRange(location: start, length: end - start), in: store.text))
        }
        func finishComposition(_ view: NSTextView) {
            guard store.isComposing, !view.hasMarkedText() else { return }
            let delta = SourceEdit.difference(from: store.text, to: view.string)
            do {
                try store.edit(range: delta.range, replacement: delta.replacement, composition: true)
                synchronizeNormalizedText(view)
            }
            catch { store.failure = error.localizedDescription; rendering = true; view.string = store.text; rendering = false }
            lineMap = SourceLineMap(view.string); document = store.selection; renderedRevision = store.revision
            if let source = view as? SourceTextView { schedule(source) }
        }
        func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange, replacementString: String?) -> Bool {
            if !rendering, !store.isComposing { pending = (affectedCharRange, replacementString ?? "") }
            return true
        }
        func textDidChange(_ notification: Notification) {
            guard !rendering, !store.isComposing, let view = notification.object as? SourceTextView else { return }
            let delta = pending.map { (range: $0.0, replacement: $0.1) } ?? SourceEdit.difference(from: store.text, to: view.string)
            pending = nil
            let currentIndex = document == store.selection && project == store.root && renderedRevision == store.revision
            do {
                try store.edit(range: delta.range, replacement: delta.replacement)
                let normalized = delta.replacement.replacingOccurrences(of: "\r\n", with: "\n")
                // Only CRLF changes an accepted replacement's representation.
                // Ordinary keystrokes must not compare the whole document.
                if normalized.utf16.count != delta.replacement.utf16.count {
                    synchronizeNormalizedText(view)
                }
                if !currentIndex || !lineMap.replace(range: delta.range, with: normalized) {
                    lineMap = SourceLineMap(store.text)
                }
            } catch {
                store.failure = error.localizedDescription; rendering = true; view.string = store.text; rendering = false
                lineMap = SourceLineMap(view.string)
            }
            document = store.selection; renderedRevision = store.revision
            schedule(view); scheduleCompletion(view); reportCaret(view)
        }
        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            guard let source = textView as? SourceTextView else { return false }
            return completion.handle(commandSelector, view: source)
        }
        func textViewDidChangeSelection(_ notification: Notification) {
            guard !rendering, let view = notification.object as? SourceTextView else { return }
            capturePresentation(view)
            scheduleCompletion(view)
            reportCaret(view); view.setNeedsDisplay(view.visibleRect, avoidAdditionalLayout: true)
            view.enclosingScrollView?.verticalRulerView?.needsDisplay = true
        }
        /// Geometry follows marked text before its single core transaction commits.
        func displayedLineMap(for view: NSTextView) -> SourceLineMap {
            store.isComposing ? SourceLineMap(view.string) : lineMap
        }
        func reportCaret(_ view: SourceTextView) {
            let position = displayedLineMap(for: view).position(at: view.selectedRange().location)
            store.reportCaret(line: position.line, column: position.column)
            if store.editorSelection != view.selectedRange() { store.editorSelection = view.selectedRange() }
        }
    }
}
