import XCTest
@testable import LaTeXIDEKit

final class ThemeContractTests: XCTestCase {
    func testEveryBundledPaletteKeepsSyntaxAndSelectionReadable() throws {
        let catalog = try ThemeCatalog.bundled()
        XCTAssertEqual(Set(catalog.palettes.map(\.id)), Set(["graphite", "ocean", "dune", "contrast"]))
        for palette in catalog.palettes {
            for isDark in [false, true] {
                let theme = palette.resolve(dark: isDark)
                for role in ["text", "secondary", "command", "environment", "delimiter", "comment", "math", "parameter", "literal", "number", "operator", "attribute"] {
                    XCTAssertGreaterThanOrEqual(theme.color(role).contrast(with: theme.color("editor")), 4.5, "\(palette.id)/\(isDark)/\(role)")
                }
                XCTAssertGreaterThanOrEqual(theme.color("text").contrast(with: theme.color("selection")), 4.5)
                XCTAssertGreaterThanOrEqual(theme.color("secondary").contrast(with: theme.color("sidebar")), 4.5)
            }
        }
    }
    @MainActor func testFixedAppearanceAndPalettePersistIndependentlyOfSystem() {
        let name = "latexide-theme-test-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        let first = EditorPreferences(defaults: defaults)
        XCTAssertEqual(first.appearance, .system)
        first.appearance = .dark; first.paletteID = "dune"
        let restored = EditorPreferences(defaults: defaults)
        XCTAssertEqual(restored.appearance, .dark)
        XCTAssertEqual(restored.paletteID, "dune")
        XCTAssertTrue(restored.appearance.isDark(systemDark: false))
        restored.appearance = .light
        XCTAssertFalse(restored.appearance.isDark(systemDark: true))
        restored.appearance = .system
        XCTAssertTrue(restored.appearance.isDark(systemDark: true))
        XCTAssertFalse(restored.appearance.isDark(systemDark: false))
    }
    @MainActor func testWorkspaceDimensionsAndCompletionChoiceSurviveRelaunch() {
        let name = "latexide-layout-test-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        let first = EditorPreferences(defaults: defaults)
        first.sidebarWidth = 310; first.outputHeight = 240; first.previewFraction = 0.48
        first.completionEnabled = false; first.inspectorTab = .assistant
        let restored = EditorPreferences(defaults: defaults)
        XCTAssertEqual(restored.sidebarWidth, 310); XCTAssertEqual(restored.outputHeight, 240)
        XCTAssertEqual(restored.previewFraction, 0.48, accuracy: 0.001)
        XCTAssertFalse(restored.completionEnabled); XCTAssertEqual(restored.inspectorTab, .assistant)
    }
}
