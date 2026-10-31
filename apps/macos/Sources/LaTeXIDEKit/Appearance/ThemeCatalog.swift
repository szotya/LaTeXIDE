import AppKit
import SwiftUI
import IDEThemeAssets

struct ThemeColor: Decodable, Equatable {
    let red: Double, green: Double, blue: Double
    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        guard value.count == 7, value.first == "#", let rgb = UInt32(value.dropFirst(), radix: 16) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "Invalid theme color"))
        }
        red = Double((rgb >> 16) & 255) / 255; green = Double((rgb >> 8) & 255) / 255; blue = Double(rgb & 255) / 255
    }
    var nsColor: NSColor { NSColor(srgbRed: red, green: green, blue: blue, alpha: 1) }
    var swiftUI: Color { Color(nsColor: nsColor) }
    var luminance: Double {
        func linear(_ c: Double) -> Double { c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    }
    func contrast(with other: ThemeColor) -> Double { (max(luminance, other.luminance) + 0.05) / (min(luminance, other.luminance) + 0.05) }
}
struct ResolvedTheme: Equatable {
    let id: String
    let dark: Bool
    let values: [String: ThemeColor]
    func color(_ role: String) -> ThemeColor { values[role] ?? values["text"]! }
    func ns(_ role: String) -> NSColor { color(role).nsColor }
    func ui(_ role: String) -> Color { color(role).swiftUI }
    func syntaxColor(kind: Int) -> NSColor {
        let roles = ["text", "command", "environment", "delimiter", "comment", "math", "parameter", "literal", "number", "operator", "attribute"]
        return ns(roles.indices.contains(kind) ? roles[kind] : "text")
    }
}
struct PaletteFamily: Decodable, Identifiable {
    let id: String, name: String, description: String
    let light: [String: ThemeColor], dark: [String: ThemeColor]
    func resolve(dark isDark: Bool) -> ResolvedTheme { ResolvedTheme(id: id, dark: isDark, values: isDark ? dark : light) }
}
struct ThemeCatalog: Decodable {
    let palettes: [PaletteFamily]
    static func bundled() throws -> ThemeCatalog { try JSONDecoder().decode(Self.self, from: ThemeAssets.paletteData()) }
    static let shared: ThemeCatalog = {
        do { return try bundled() } catch { fatalError("Bundled IDE palettes are missing or invalid: \(error)") }
    }()
    func palette(_ id: String) -> PaletteFamily { palettes.first(where: { $0.id == id }) ?? palettes[0] }
}
public enum AppearanceMode: String, CaseIterable, Identifiable {
    case system, light, dark
    public var id: String { rawValue }
    var title: String { switch self { case .system: "Rendszer"; case .light: "Világos"; case .dark: "Sötét" } }
    var symbol: String { switch self { case .system: "desktopcomputer"; case .light: "sun.max"; case .dark: "moon" } }
    func isDark(systemDark: Bool) -> Bool { self == .system ? systemDark : self == .dark }
    var colorScheme: ColorScheme? { switch self { case .system: nil; case .light: .light; case .dark: .dark } }
}
@MainActor public final class EditorPreferences: ObservableObject {
    private let defaults: UserDefaults
    @Published public var appearance: AppearanceMode { didSet { defaults.set(appearance.rawValue, forKey: "ide.appearance") } }
    @Published public var paletteID: String { didSet { defaults.set(paletteID, forKey: "ide.palette") } }
    @Published var fontSize: Double { didSet { defaults.set(fontSize, forKey: "ide.fontSize") } }
    @Published var completionEnabled: Bool { didSet { defaults.set(completionEnabled, forKey: "ide.completion") } }
    @Published var sidebarWidth: Double { didSet { defaults.set(sidebarWidth, forKey: "ide.sidebarWidth") } }
    @Published var outputHeight: Double { didSet { defaults.set(outputHeight, forKey: "ide.outputHeight") } }
    @Published var previewFraction: Double { didSet { defaults.set(previewFraction, forKey: "ide.previewFraction") } }
    @Published var inspectorTab: InspectorTab { didSet { defaults.set(inspectorTab.rawValue, forKey: "ide.inspectorTab") } }
    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        appearance = AppearanceMode(rawValue: defaults.string(forKey: "ide.appearance") ?? "system") ?? .system
        paletteID = defaults.string(forKey: "ide.palette") ?? "graphite"
        let stored = defaults.double(forKey: "ide.fontSize")
        fontSize = stored == 0 ? 14 : min(22, max(11, stored))
        completionEnabled = defaults.object(forKey: "ide.completion") == nil ? true : defaults.bool(forKey: "ide.completion")
        func number(_ key: String, fallback: Double, range: ClosedRange<Double>) -> Double {
            guard defaults.object(forKey: key) != nil else { return fallback }
            let value = defaults.double(forKey: key)
            return value.isFinite ? min(range.upperBound, max(range.lowerBound, value)) : fallback
        }
        sidebarWidth = number("ide.sidebarWidth", fallback: 230, range: 190...420)
        outputHeight = number("ide.outputHeight", fallback: 160, range: 95...600)
        previewFraction = number("ide.previewFraction", fallback: 0.42, range: 0.25...0.70)
        inspectorTab = InspectorTab(rawValue: defaults.string(forKey: "ide.inspectorTab") ?? "pdf") ?? .pdf
    }
    func theme(systemDark: Bool) -> ResolvedTheme { ThemeCatalog.shared.palette(paletteID).resolve(dark: appearance.isDark(systemDark: systemDark)) }
}
