import XCTest
import AppKit
import SwiftUI
import PDFKit
@testable import LaTeXIDEKit

/// Opt-in, controlled native render artifacts. Separate from live GUI acceptance.
final class WorkspaceRenderTests: XCTestCase {
    @MainActor func testRenderPaletteWorkspaces() throws {
        guard ProcessInfo.processInfo.environment["LATEXIDE_RENDER_PREVIEWS"] == "1" else {
            throw XCTSkip("Opt-in native visual artifact generation")
        }
        _ = NSApplication.shared
        var repository = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repository.deleteLastPathComponent() }
        let project = repository.appendingPathComponent("evidence/ui-redesign/demo-project")
        let buildData = try Data(contentsOf: repository.appendingPathComponent("evidence/ui-redesign/demo-build.json"))
        let build = try JSONSerialization.jsonObject(with: buildData) as! [String: Any]
        let store = WorkspaceStore(); store.open(project)
        XCTAssertNil(store.failure)
        store.select("chapters/modell.tex"); store.select("references.bib"); store.select("main.tex")
        // Controlled presentation uses an actual separately compiled demo PDF/log.
        store.pdf = URL(fileURLWithPath: build["pdf_path"] as! String)
        store.buildState = build["state"] as! String
        store.buildLog = build["log"] as! String
        store.bottomPanel = .problems
        let suite = "latexide-render-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let preferences = EditorPreferences(defaults: defaults)
        let rect = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let window = NSWindow(contentRect: rect, styleMask: [.titled, .closable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.titleVisibility = .hidden; window.titlebarAppearsTransparent = true
        let host = NSHostingView(rootView: WorkspaceView(store: store, preferences: preferences))
        host.frame = rect; window.contentView = host
        window.setFrameOrigin(NSPoint(x: -5000, y: -5000)); window.orderBack(nil)
        defer { window.close() }
        for (palette, appearance) in [("graphite", AppearanceMode.dark), ("ocean", .dark), ("dune", .light), ("graphite", .light)] {
            preferences.paletteID = palette; preferences.appearance = appearance
            window.appearance = NSAppearance(named: appearance == .dark ? .darkAqua : .aqua)
            host.layoutSubtreeIfNeeded()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
            host.layoutSubtreeIfNeeded(); host.displayIfNeeded()
            let target = host
            guard let bitmap = target.bitmapImageRepForCachingDisplay(in: target.bounds) else { XCTFail("Native bitmap unavailable"); return }
            target.cacheDisplay(in: target.bounds, to: bitmap)
            // PDFKit's tiled GPU layers do not participate in off-screen cacheDisplay.
            // Render the real PDFPage into the same native page coordinates for this
            // controlled artifact; this is not live screen-capture evidence.
            drawPDFPages(in: host, into: bitmap)
            guard let png = bitmap.representation(using: .png, properties: [:]) else { XCTFail("PNG unavailable"); return }
            let output = repository.appendingPathComponent("evidence/ui-redesign/workspace-\(palette)-\(appearance.rawValue).png")
            try png.write(to: output)
            XCTAssertGreaterThan(png.count, 10000)
        }
        XCTAssertTrue(store.requestTermination())
    }
    @MainActor private func drawPDFPages(in root: NSView, into bitmap: NSBitmapImageRep) {
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        guard let graphics = NSGraphicsContext(bitmapImageRep: bitmap) else { return }
        let context = graphics.cgContext
        context.saveGState(); defer { context.restoreGState() }
        context.concatenate(context.ctm.inverted())
        context.scaleBy(x: CGFloat(bitmap.pixelsWide) / root.bounds.width, y: CGFloat(bitmap.pixelsHigh) / root.bounds.height)
        for view in descendants(root).compactMap({ $0 as? PDFView }) {
            let region = view.convert(view.bounds, to: root)
            let clip = CGRect(x: region.minX, y: root.bounds.height - region.maxY, width: region.width, height: region.height)
            context.saveGState(); context.clip(to: clip)
            for page in view.visiblePages {
                let bounds = page.bounds(for: .cropBox)
                let rect = view.convert(view.convert(bounds, from: page), to: root)
                let destination = CGRect(x: rect.minX, y: root.bounds.height - rect.maxY, width: rect.width, height: rect.height)
                context.saveGState()
                context.setFillColor(NSColor.white.cgColor); context.fill(destination)
                context.translateBy(x: destination.minX, y: destination.minY)
                context.scaleBy(x: destination.width / bounds.width, y: destination.height / bounds.height)
                context.translateBy(x: -bounds.minX, y: -bounds.minY)
                page.draw(with: .cropBox, to: context)
                context.restoreGState()
            }
            context.restoreGState()
        }
    }
}
