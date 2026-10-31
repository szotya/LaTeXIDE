// swift-tools-version: 5.10
import PackageDescription
let package = Package(name: "IDEThemeAssets", products: [
    .library(name: "IDEThemeAssets", targets: ["IDEThemeAssets"])
], targets: [.target(name: "IDEThemeAssets", resources: [.copy("Resources/editor-themes.json")])])
