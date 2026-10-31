import Foundation
public enum ThemeAssets {
    public static func paletteData() throws -> Data {
        guard let url = Bundle.module.url(forResource: "editor-themes", withExtension: "json") else {
            throw CocoaError(.fileNoSuchFile)
        }
        return try Data(contentsOf: url)
    }
}
