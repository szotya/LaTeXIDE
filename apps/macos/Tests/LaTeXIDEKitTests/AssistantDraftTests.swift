import AppKit
import SwiftUI
import XCTest
@testable import LaTeXIDEKit

private struct DraftTestWorkspace: View {
    let store: WorkspaceStore
    @ObservedObject var session: AssistantSession
    var body: some View {
        Group {
            if store.pdfVisible { InspectorPanel(store: store, session: session) }
            else { Color.clear }
        }.frame(width: 440,height: 700)
    }
}
final class AssistantDraftTests: XCTestCase {
    @MainActor func testUnsentPromptSurvivesPDFAndHiddenInspectorRoundTrips() throws {
        _ = NSApplication.shared
        let store = WorkspaceStore(), session = AssistantSession()
        store.inspector = .assistant
        let host = NSHostingView(rootView: DraftTestWorkspace(store: store, session: session))
        let window = NSWindow(contentRect: NSRect(x: -6000,y: -6000,width: 440,height: 700), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = host
        defer { window.close(); session.disconnect() }
        func settle() { host.layoutSubtreeIfNeeded(); RunLoop.current.run(until: Date().addingTimeInterval(0.05)); host.layoutSubtreeIfNeeded() }
        func editor(_ view: NSView) -> NSTextView? {
            if let value = view as? NSTextView, value.isEditable { return value }
            return view.subviews.lazy.compactMap { editor($0) }.first
        }
        settle()
        let first = try XCTUnwrap(editor(host)); window.makeFirstResponder(first)
        first.insertText("Kérlek magyarázd ezt a képletet.", replacementRange: NSRange(location: 0,length: 0)); settle()
        session.draftSelection = "\\frac{1}{2}"; session.draftDiagnostics = "main.tex:5 Undefined control sequence"
        XCTAssertEqual(first.string, "Kérlek magyarázd ezt a képletet.")
        store.inspector = .pdf; settle(); store.inspector = .assistant; settle()
        XCTAssertEqual(try XCTUnwrap(editor(host)).string, "Kérlek magyarázd ezt a képletet.")
        XCTAssertEqual(session.draftSelection, "\\frac{1}{2}"); XCTAssertEqual(session.draftDiagnostics, "main.tex:5 Undefined control sequence")
        store.pdfVisible = false; settle(); store.pdfVisible = true; settle()
        XCTAssertEqual(try XCTUnwrap(editor(host)).string, "Kérlek magyarázd ezt a képletet.")
        XCTAssertEqual(session.draftSelection, "\\frac{1}{2}"); XCTAssertEqual(session.draftDiagnostics, "main.tex:5 Undefined control sequence")
    }
}
