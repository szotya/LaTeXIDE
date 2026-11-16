import AppKit
import SwiftUI
import XCTest
@testable import LaTeXIDEKit

/// Local hidden-window controls. Never orders, activates, or makes a window key.
/// Rendering is opt-in and is not live window-manager/AX acceptance.
final class WindowToolbarLayoutTests: XCTestCase {
    @MainActor private final class Fixture {
        let project: URL
        let original = "\\documentclass{article}\n\\begin{document}\nSzintetikus kutatási jegyzet: árvíztűrő tükörfúrógép — λ 🧪\n\\end{document}\n"
        let store = WorkspaceStore()
        let defaults: UserDefaults
        let suite = "hu.latexide.ui57.\(UUID().uuidString)"
        let preferences: EditorPreferences
        let window: NSWindow
        let host: NSHostingView<WorkspaceView>
        var requestedFrame: NSRect = .zero
        var lastGeometry: [String: Any] = [:]
        var actionEvidence: [[String: Any]] = []

        init(width: Int = 1280, height: Int = 800, appearance: AppearanceMode = .light) throws {
            _ = NSApplication.shared
            project = FileManager.default.temporaryDirectory.appendingPathComponent("ui57-fixture-\(UUID().uuidString)")
            try FileManager.default.createDirectory(at: project, withIntermediateDirectories: true)
            try Data(original.utf8).write(to: project.appendingPathComponent("main.tex"))
            try Data("\\section{Második szintetikus fájl}\n".utf8).write(to: project.appendingPathComponent("notes.tex"))
            // A real owned setup blocker keeps the existing recovery warning visible.
            let metadata = project.appendingPathComponent(".latexide")
            try FileManager.default.createDirectory(at: metadata, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            try Data("ui57 owned recovery blocker".utf8).write(to: metadata.appendingPathComponent("recovery"))
            defaults = UserDefaults(suiteName: suite)!
            preferences = EditorPreferences(defaults: defaults)
            preferences.appearance = appearance; preferences.sidebarWidth = 210; preferences.outputHeight = 110
            store.open(project)
            XCTAssertNil(store.failure); XCTAssertNotNil(store.recoveryWarning)
            try store.edit(range: NSRange(location: 0, length: 0), replacement: "% El nem mentett Unicode: őű λ 🧪\n")
            let rect = NSRect(x: -6000, y: -6000, width: width, height: height)
            var style: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .resizable]
            #if UI57_BASELINE
            style.insert(.fullSizeContentView)
            #endif
            window = NSWindow(contentRect: rect, styleMask: style, backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            host = NSHostingView(rootView: WorkspaceView(store: store, preferences: preferences))
            host.sizingOptions = []; window.contentView = host
            requestedFrame = rect; window.setFrame(rect, display: false)
            settle()
        }
        func settle() {
            window.contentView?.superview?.layoutSubtreeIfNeeded()
            RunLoop.current.run(until: Date().addingTimeInterval(0.08))
            if window.frame != requestedFrame { window.setFrame(requestedFrame, display: false) }
            window.contentView?.superview?.layoutSubtreeIfNeeded()
            // A hidden hosting view has no window-server display pass. Explicitly
            // materialize its SwiftUI layout before asking for local controls.
            if let frame = window.contentView?.superview {
                func draw(_ view: NSView) {
                    view.layoutSubtreeIfNeeded(); view.display()
                    view.subviews.forEach(draw)
                }
                draw(frame)
            }
        }
        func resize(to rect: NSRect) { requestedFrame = rect; window.setFrame(rect, display: false) }
        func writeGeometry(_ metadata: [String: Any], stage: String) throws {
            lastGeometry = metadata
            guard let directory = ProcessInfo.processInfo.environment["UI57_RENDER_DIR"] else { return }
            let caseID = ProcessInfo.processInfo.environment["UI57_CASE"] ?? suite
            let output = URL(fileURLWithPath: directory).appendingPathComponent("geometry").appendingPathComponent(caseID)
            try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
            let filename = "\(Int(requestedFrame.width))x\(Int(requestedFrame.height))-\(preferences.appearance.rawValue).json"
            var value = metadata; value["stage"] = stage
            try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent(filename))
        }
        func stateEvidence() -> [String: Any] {
            ["text": store.text, "revision": store.revision, "dirty": store.dirty, "canUndo": store.canUndo, "canRedo": store.canRedo,
             "sidebar": String(describing: store.sidebar), "inspector": String(describing: store.inspector), "pdfVisible": store.pdfVisible,
             "quickOpen": String(describing: store.quickOpen), "mainFile": store.mainFile, "buildState": store.buildState,
             "operation": store.operation ?? "", "recoveryWarning": store.recoveryWarning ?? ""]
        }
        func recordPress(_ button: NSButton, before: [String: Any], delivered: Bool) {
            actionEvidence.append(["identifier": button.accessibilityIdentifier(), "object_class": String(describing: type(of: button)),
                                   "target_class": String(describing: type(of: button.target!)), "action": NSStringFromSelector(button.action!),
                                   "actual_owned_window": button.window === window, "native_dispatch_result": delivered,
                                   "before": before, "after": stateEvidence()])
            guard let directory = ProcessInfo.processInfo.environment["UI57_RENDER_DIR"] else { return }
            do {
                let output = URL(fileURLWithPath: directory).appendingPathComponent("actions")
                try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
                let name = ProcessInfo.processInfo.environment["UI57_CASE"] ?? suite
                try JSONSerialization.data(withJSONObject: actionEvidence, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent(name + ".json"))
            } catch { XCTFail("Native action evidence write failed: \(error)") }
        }
        func close() {
            window.contentView = nil; window.close()
            store.save(); XCTAssertTrue(store.requestTermination())
            defaults.removePersistentDomain(forName: suite)
            try? FileManager.default.removeItem(at: project)
        }
        func assertHidden(file: StaticString = #filePath, line: UInt = #line) {
            XCTAssertFalse(window.isVisible, file: file, line: line)
            XCTAssertFalse(window.isKeyWindow, file: file, line: line)
            XCTAssertFalse(window.isMainWindow, file: file, line: line)
        }
    }

    @MainActor private func descendants(_ root: NSView) -> [NSView] { [root] + root.subviews.flatMap(descendants) }

    #if !UI57_BASELINE
    @MainActor private func controls(_ fixture: Fixture) throws -> TitlebarHostingView {
        let frame = try XCTUnwrap(fixture.window.contentView?.superview)
        return try XCTUnwrap(descendants(frame).compactMap { $0 as? TitlebarHostingView }.first)
    }
    /// Resolve the actual live native controls owned by the SwiftUI titlebar.
    /// Each object is a real button with its real target/action; no AX proxy,
    /// label guess, geometry-derived callback, stored test action, or fake press.
    @MainActor private struct Control {
        let object: NSButton
        let fixture: Fixture
        func accessibilityFrame() -> NSRect { object.accessibilityFrame() }
        func isAccessibilityEnabled() -> Bool { object.isAccessibilityEnabled() }
        func accessibilityLabel() -> String? { object.accessibilityLabel() }
        func accessibilityPerformPress() -> Bool {
            let before = fixture.stateEvidence()
            let delivered = object.accessibilityPerformPress()
            fixture.recordPress(object, before: before, delivered: delivered)
            return delivered
        }
    }
    @MainActor private func button(_ id: String, in fixture: Fixture) throws -> Control {
        let root = try controls(fixture)
        let matches = descendants(root).compactMap { $0 as? NSButton }.filter { $0.accessibilityIdentifier() == id }
        XCTAssertEqual(matches.count, 1, "One actual native control must own identifier \(id)")
        let actual = try XCTUnwrap(matches.first, "Missing actual native control: \(id)")
        XCTAssertTrue(actual.isDescendant(of: root)); XCTAssertTrue(actual.window === fixture.window)
        XCTAssertNotNil(actual.target); XCTAssertNotNil(actual.action)
        _ = try XCTUnwrap(actual.target); _ = try XCTUnwrap(actual.action)
        return Control(object: actual, fixture: fixture)
    }
    @MainActor private func geometry(_ fixture: Fixture) throws -> [String: Any] {
        let window = fixture.window
        let frame = try XCTUnwrap(window.contentView?.superview)
        let chrome = try controls(fixture)
        let close = try XCTUnwrap(window.standardWindowButton(.closeButton))
        let zoom = try XCTUnwrap(window.standardWindowButton(.zoomButton))
        let closeRect = close.convert(close.bounds, to: frame)
        let zoomRect = zoom.convert(zoom.bounds, to: frame)
        let chromeRect = chrome.convert(chrome.bounds, to: frame)
        XCTAssertEqual(window.titlebarAccessoryViewControllers.count, 1)
        XCTAssertFalse(window.isMovableByWindowBackground)
        XCTAssertGreaterThanOrEqual(chromeRect.minX, zoomRect.maxX + 8)
        XCTAssertEqual(chromeRect.midY, closeRect.midY, accuracy: 1)
        XCTAssertLessThanOrEqual(chromeRect.maxX, frame.bounds.maxX)
        XCTAssertTrue(frame.hitTest(closeRect.center) === close, "Native close button is not covered")
        let ids = ["workspace-project", "workspace-quick-open", "workspace-main-document", "workspace-build", "workspace-sidebar", "workspace-pdf", "workspace-assistant", "workspace-appearance"]
        var regions: [[String: Any]] = []
        var metadata: [String: Any] = ["requested_outer": NSStringFromRect(fixture.requestedFrame), "actual_outer": NSStringFromRect(window.frame), "frame_bounds": NSStringFromRect(frame.bounds), "content_frame": NSStringFromRect(fixture.host.frame), "native_close": NSStringFromRect(closeRect), "native_zoom": NSStringFromRect(zoomRect), "controls": NSStringFromRect(chromeRect), "appearance": fixture.preferences.appearance.rawValue, "visible": window.isVisible, "key": window.isKeyWindow, "main": window.isMainWindow]
        try fixture.writeGeometry(metadata, stage: "native-before-control-discovery")
        XCTAssertEqual(window.frame.size, fixture.requestedFrame.size)
        XCTAssertEqual(frame.bounds.size, fixture.requestedFrame.size)
        var previous: NSRect?
        for id in ids {
            let element: Control
            do { element = try button(id, in: fixture) }
            catch {
                regions.append(["id": id, "status": "FAIL_OR_ERROR", "error": String(describing: error)])
                metadata["items"] = regions; try fixture.writeGeometry(metadata, stage: "control-discovery-error")
                continue
            }
            let screen = element.accessibilityFrame()
            let rect = frame.convert(window.convertFromScreen(screen), from: nil)
            XCTAssertGreaterThan(rect.width, 10, id)
            XCTAssertGreaterThan(rect.height, 10, id)
            let nativeRect = element.object.convert(element.object.bounds, to: frame)
            let hit = frame.hitTest(nativeRect.center)
            XCTAssertTrue(hit === element.object || hit?.isDescendant(of: element.object) == true, "Actual control hit region: \(id)")
            var fitting: [String: Any] = [:]
            if id == "workspace-build" {
                let native = try XCTUnwrap(element.object as? TitlebarNativeButton)
                let label = native.labelHost.intrinsicContentSize
                XCTAssertTrue(native.usesIntrinsicWidth)
                XCTAssertGreaterThan(label.width, 10)
                XCTAssertGreaterThanOrEqual(native.bounds.width + 1, label.width, "Visible Build label must fit its actual hit bounds")
                var proposals: [[String: Any]] = []
                let widths: [CGFloat?] = [nil, 0, 1, 10]
                for width in widths {
                    let measured = native.sizeThatFits(ProposedViewSize(width: width, height: 28))
                    XCTAssertGreaterThanOrEqual(measured.width, label.width)
                    XCTAssertEqual(measured.width, native.intrinsicContentSize.width, accuracy: 0.5)
                    proposals.append(["offered_width": width.map { String(describing: $0) } ?? "unspecified", "measured": NSStringFromSize(measured)])
                }
                fitting = ["label_intrinsic": NSStringFromSize(label), "button_intrinsic": NSStringFromSize(native.intrinsicContentSize), "native_bounds": NSStringFromRect(native.bounds), "proposals": proposals]
            }
            XCTAssertEqual(rect.midY, closeRect.midY, accuracy: 2, id)
            XCTAssertGreaterThanOrEqual(rect.minX, chromeRect.minX - 1, id)
            XCTAssertLessThanOrEqual(rect.maxX, chromeRect.maxX + 1, id)
            if let previous { XCTAssertLessThanOrEqual(previous.maxX, rect.minX + 1, "Overlapping controls: \(id)") }
            previous = rect
            regions.append(["id": id, "frame": NSStringFromRect(rect), "enabled": element.isAccessibilityEnabled(), "object_class": String(describing: type(of: element.object)), "target_class": String(describing: type(of: element.object.target!)), "action": NSStringFromSelector(element.object.action!), "actual_window_match": element.object.window === window, "native_frame": NSStringFromRect(nativeRect), "actual_hit": hit === element.object || hit?.isDescendant(of: element.object) == true, "fitting": fitting])
        }
        let gaps = descendants(chrome).compactMap { $0 as? TitlebarDragView }
        XCTAssertEqual(gaps.count, 2)
        XCTAssertFalse(chrome.mouseDownCanMoveWindow)
        for gap in gaps {
            XCTAssertTrue(gap.mouseDownCanMoveWindow)
            XCTAssertGreaterThanOrEqual(gap.bounds.width, 8)
            XCTAssertTrue(chrome.hitTest(gap.convert(gap.bounds.center, to: chrome)) === gap)
        }
        metadata["items"] = regions
        try fixture.writeGeometry(metadata, stage: "all-native-and-control-guards-evaluated")
        return metadata
    }

    @MainActor func testNativeGeometryLongNamesResizeAndLifecycle() throws {
        let fixture = try Fixture(); defer { fixture.close() }
        let actualBuild = try XCTUnwrap(try button("workspace-build", in: fixture).object as? TitlebarNativeButton)
        let disabledSizing = TitlebarButtonLabel(rootView: actualBuild.labelHost.rootView)
        disabledSizing.sizingOptions = []
        let oldAutoLayoutFit = disabledSizing.fittingSize
        let actualIntrinsic = actualBuild.labelHost.intrinsicContentSize
        XCTAssertGreaterThan(actualIntrinsic.width, 10)
        if let directory = ProcessInfo.processInfo.environment["UI57_RENDER_DIR"] {
            let output = URL(fileURLWithPath: directory).appendingPathComponent("sizing-controls")
            try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
            let observation: [String: Any] = ["scope": "real SwiftUI label measurement only; no AX/action acceptance", "same_root_label": true, "disabled_intrinsic_auto_layout_fit": NSStringFromSize(oldAutoLayoutFit), "actual_enabled_label_intrinsic": NSStringFromSize(actualIntrinsic), "native_button_intrinsic": NSStringFromSize(actualBuild.intrinsicContentSize)]
            try JSONSerialization.data(withJSONObject: observation, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent("label-measurement.json"))
        }
        fixture.store.mainFile = String(repeating: "hosszú-őű-", count: 30) + ".tex"
        for size in [NSSize(width: 1080, height: 680), NSSize(width: 1440, height: 900), NSSize(width: 1280, height: 800)] {
            fixture.resize(to: NSRect(origin: fixture.window.frame.origin, size: size))
            fixture.settle(); _ = try geometry(fixture); fixture.assertHidden()
        }
        // Repeated SwiftUI updates must not add another accessory or lose state.
        fixture.preferences.paletteID = "ocean"; fixture.preferences.appearance = .dark; fixture.settle()
        XCTAssertEqual(fixture.window.titlebarAccessoryViewControllers.count, 1)
        fixture.window.contentView = nil; fixture.settle()
        XCTAssertEqual(fixture.window.titlebarAccessoryViewControllers.count, 0)
    }

    @MainActor func testActualQuickOpenAndPanelActionsPreserveDirtyDocumentAndDraft() throws {
        let fixture = try Fixture(); defer { fixture.close() }
        let store = fixture.store, text = store.text, revision = store.revision
        let warning = store.recoveryWarning
        XCTAssertTrue(try button("workspace-sidebar", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertNil(store.sidebar)
        XCTAssertTrue(try button("workspace-sidebar", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertEqual(store.sidebar, .project)
        XCTAssertTrue(try button("workspace-assistant", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertEqual(store.inspector, .assistant); XCTAssertTrue(store.pdfVisible)
        let composer = try XCTUnwrap(descendants(fixture.host).compactMap { $0 as? NSTextView }.first { $0.isEditable && !($0 is SourceTextView) })
        XCTAssertTrue(fixture.window.makeFirstResponder(composer))
        composer.insertText("El nem küldött szintetikus kérdés: λ őű 🧪", replacementRange: NSRange(location: 0, length: 0))
        fixture.settle(); let draft = composer.string
        XCTAssertTrue(try button("workspace-pdf", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertEqual(store.inspector, .pdf)
        XCTAssertTrue(try button("workspace-pdf", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertFalse(store.pdfVisible)
        XCTAssertTrue(try button("workspace-assistant", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertEqual(try XCTUnwrap(descendants(fixture.host).compactMap { $0 as? NSTextView }.first { $0.isEditable && !($0 is SourceTextView) }).string, draft)
        XCTAssertTrue(try button("workspace-quick-open", in: fixture).accessibilityPerformPress()); fixture.settle()
        XCTAssertEqual(store.quickOpen, .files)
        let search = try XCTUnwrap(descendants(fixture.host).compactMap { $0 as? NSTextField }.first { $0.isEditable })
        search.stringValue = "notes"; search.sendAction(search.action, to: search.target)
        store.quickOpen = nil; fixture.resize(to: NSRect(x: -6000, y: -6000, width: 1080, height: 680)); fixture.settle()
        XCTAssertEqual(store.text, text); XCTAssertEqual(store.revision, revision)
        XCTAssertTrue(store.dirty); XCTAssertTrue(store.canUndo); XCTAssertFalse(store.canRedo)
        XCTAssertEqual(store.recoveryWarning, warning)
        XCTAssertEqual(try String(contentsOf: fixture.project.appendingPathComponent("main.tex"), encoding: .utf8), fixture.original)
        XCTAssertEqual(try XCTUnwrap(descendants(fixture.host).compactMap { $0 as? NSTextView }.first { $0.isEditable && !($0 is SourceTextView) }).string, draft)
        store.undo(); XCTAssertEqual(store.text, fixture.original); XCTAssertTrue(store.canRedo)
        store.redo(); XCTAssertEqual(store.text, text); XCTAssertTrue(store.dirty)
        fixture.assertHidden()
    }

    @MainActor func testNativeQueryAndPromptUndoDoNotWriteDocumentHistory() throws {
        let fixture = try Fixture(); defer { fixture.close() }
        let text = fixture.store.text, revision = fixture.store.revision
        for id in ["query", "prompt"] {
            let field = NSTextView(frame: NSRect(x: 0, y: 0, width: 300, height: 40)); field.allowsUndo = true
            fixture.host.addSubview(field); defer { field.removeFromSuperview() }
            XCTAssertTrue(fixture.window.makeFirstResponder(field))
            let manager = try XCTUnwrap(field.undoManager); manager.removeAllActions(); manager.groupsByEvent = false
            manager.beginUndoGrouping(); field.insertText(id + " őű", replacementRange: NSRange(location: 0, length: 0)); manager.endUndoGrouping()
            fixture.store.undoCommand(in: fixture.window); XCTAssertEqual(field.string, "")
            fixture.store.redoCommand(in: fixture.window); XCTAssertEqual(field.string, id + " őű")
            XCTAssertEqual(fixture.store.text, text); XCTAssertEqual(fixture.store.revision, revision); XCTAssertTrue(fixture.store.dirty)
        }
        fixture.assertHidden()
    }

    @MainActor func testBuildEnabledStopStateAndSystemAppearance() throws {
        let fixture = try Fixture(appearance: .system); defer { fixture.close() }
        XCTAssertNil(fixture.window.appearance)
        XCTAssertTrue(try button("workspace-build", in: fixture).isAccessibilityEnabled())
        fixture.store.isComposing = true; fixture.settle()
        XCTAssertFalse(try button("workspace-build", in: fixture).isAccessibilityEnabled())
        fixture.store.isComposing = false; fixture.store.buildState = "running"; fixture.settle()
        let stop = try button("workspace-build", in: fixture)
        XCTAssertTrue(stop.accessibilityLabel()?.contains("Leállítás") == true)
        let nativeStop = try XCTUnwrap(stop.object as? TitlebarNativeButton)
        XCTAssertGreaterThan(nativeStop.labelHost.intrinsicContentSize.width, 10)
        XCTAssertGreaterThanOrEqual(nativeStop.bounds.width + 1, nativeStop.labelHost.intrinsicContentSize.width)
        // No operation exists in this presentation fixture: cancel is a safe no-op.
        XCTAssertTrue(stop.accessibilityPerformPress())
        fixture.store.buildState = ""; fixture.preferences.appearance = .dark; fixture.settle()
        XCTAssertNotNil(fixture.window.appearance)
        fixture.preferences.appearance = .system; fixture.settle(); XCTAssertNil(fixture.window.appearance)
        fixture.assertHidden()
    }

    @MainActor func testPublisherDisabledPressAndNonfiniteSizeProposalsStayBounded() throws {
        let fixture = try Fixture()
        defer { fixture.store.isComposing = false; fixture.close() }
        let text = fixture.store.text, revision = fixture.store.revision
        fixture.store.isComposing = true; fixture.settle()
        let actual = try XCTUnwrap(try button("workspace-build", in: fixture).object as? TitlebarNativeButton)
        XCTAssertFalse(actual.isEnabled)
        XCTAssertFalse(actual.accessibilityPerformPress())
        XCTAssertEqual(fixture.store.text, text); XCTAssertEqual(fixture.store.revision, revision)
        XCTAssertTrue(fixture.store.dirty); XCTAssertEqual(fixture.store.buildState, "")
        for width in [CGFloat(-1), .nan, .infinity, -.infinity] {
            let measured = actual.sizeThatFits(ProposedViewSize(width: width, height: 28))
            XCTAssertTrue(measured.width.isFinite); XCTAssertGreaterThan(measured.width, 10)
            XCTAssertEqual(measured.width, actual.intrinsicContentSize.width, accuracy: 0.5)
        }
        fixture.assertHidden()
    }

    @MainActor func testPublisherNativeMainDocumentCallbackPreservesDirtyTextAndDraft() throws {
        let fixture = try Fixture(); defer { fixture.close() }
        let text = fixture.store.text, revision = fixture.store.revision
        let warning = fixture.store.recoveryWarning
        XCTAssertTrue(try button("workspace-assistant", in: fixture).accessibilityPerformPress()); fixture.settle()
        let composer = try XCTUnwrap(descendants(fixture.host).compactMap { $0 as? NSTextView }.first { $0.isEditable && !($0 is SourceTextView) })
        XCTAssertTrue(fixture.window.makeFirstResponder(composer))
        composer.insertText("Saját callback kontroll: őű λ 🧪", replacementRange: NSRange(location: 0, length: 0))
        let draft = composer.string
        let picker = try XCTUnwrap(try button("workspace-main-document", in: fixture).object as? NSPopUpButton)
        let index = try XCTUnwrap(picker.menu?.items.firstIndex { ($0.representedObject as? String) == "notes.tex" })
        picker.selectItem(at: index)
        let before = fixture.stateEvidence()
        let action = try XCTUnwrap(picker.action); _ = try XCTUnwrap(picker.target)
        let delivered = picker.sendAction(action, to: picker.target)
        fixture.recordPress(picker, before: before, delivered: delivered)
        XCTAssertTrue(delivered); fixture.settle()
        XCTAssertEqual(fixture.store.mainFile, "notes.tex")
        XCTAssertEqual(fixture.store.text, text); XCTAssertEqual(fixture.store.revision, revision)
        XCTAssertTrue(fixture.store.dirty); XCTAssertTrue(fixture.store.canUndo)
        XCTAssertEqual(fixture.store.recoveryWarning, warning)
        XCTAssertEqual(try String(contentsOf: fixture.project.appendingPathComponent("main.tex"), encoding: .utf8), fixture.original)
        XCTAssertEqual(try XCTUnwrap(descendants(fixture.host).compactMap { $0 as? NSTextView }.first { $0.isEditable && !($0 is SourceTextView) }).string, draft)
        fixture.assertHidden()
    }

    #endif

    @MainActor private func render(width: Int, height: Int, appearance: AppearanceMode) throws {
        guard let directory = ProcessInfo.processInfo.environment["UI57_RENDER_DIR"] else { throw XCTSkip("Set UI57_RENDER_DIR for controlled hidden renders") }
        let fixture = try Fixture(width: width, height: height, appearance: appearance); defer { fixture.close() }
        let frame = try XCTUnwrap(fixture.window.contentView?.superview)
        var metadata: [String: Any] = ["requested_outer_size": "\(width)x\(height)", "actual_window": NSStringFromRect(fixture.window.frame), "content": NSStringFromRect(fixture.host.frame), "appearance": appearance.rawValue, "visible": fixture.window.isVisible, "key": fixture.window.isKeyWindow, "main": fixture.window.isMainWindow]
        #if UI57_BASELINE
        metadata["variant"] = "baseline"
        #else
        metadata["variant"] = "candidate"
        #endif
        let output = URL(fileURLWithPath: directory); try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
        try fixture.writeGeometry(metadata, stage: "render-native-before-control-discovery")
        XCTAssertEqual(fixture.window.frame.size, NSSize(width: width, height: height))
        XCTAssertEqual(frame.bounds.size, NSSize(width: width, height: height))
        for scale in [1, 2] {
            let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(frame.bounds.width) * scale, pixelsHigh: Int(frame.bounds.height) * scale, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
            bitmap.size = frame.bounds.size
            XCTAssertEqual(bitmap.pixelsWide, width * scale); XCTAssertEqual(bitmap.pixelsHigh, height * scale)
            frame.cacheDisplay(in: frame.bounds, to: bitmap)
            let data = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
            XCTAssertGreaterThan(data.count, 10000)
            let name = "\(width)x\(height)-\(appearance.rawValue)-\(scale)x.png"
            try data.write(to: output.appendingPathComponent(name))
        }
        #if !UI57_BASELINE
        metadata["geometry"] = try geometry(fixture)
        #endif
        try JSONSerialization.data(withJSONObject: metadata, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent("\(width)x\(height)-\(appearance.rawValue).json"))
        XCTAssertTrue(fixture.store.dirty); XCTAssertTrue(fixture.store.canUndo)
        XCTAssertEqual(try String(contentsOf: fixture.project.appendingPathComponent("main.tex"), encoding: .utf8), fixture.original)
        fixture.assertHidden()
    }
    @MainActor func testRender1080Light() throws { try render(width: 1080, height: 680, appearance: .light) }
    @MainActor func testRender1080Dark() throws { try render(width: 1080, height: 680, appearance: .dark) }
    @MainActor func testRender1080System() throws { try render(width: 1080, height: 680, appearance: .system) }
    @MainActor func testRender1280Light() throws { try render(width: 1280, height: 800, appearance: .light) }
    @MainActor func testRender1280Dark() throws { try render(width: 1280, height: 800, appearance: .dark) }
    @MainActor func testRender1280System() throws { try render(width: 1280, height: 800, appearance: .system) }
    @MainActor func testRender1440Light() throws { try render(width: 1440, height: 900, appearance: .light) }
    @MainActor func testRender1440Dark() throws { try render(width: 1440, height: 900, appearance: .dark) }
    @MainActor func testRender1440System() throws { try render(width: 1440, height: 900, appearance: .system) }
}

private extension NSRect { var center: NSPoint { NSPoint(x: midX, y: midY) } }
