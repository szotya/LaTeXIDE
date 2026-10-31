// swift-tools-version: 5.10
import PackageDescription
import Foundation
let repository = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
let package = Package(name: "LaTeXIDE", platforms: [.macOS(.v14)], products: [
    .executable(name: "LaTeXIDE", targets: ["LaTeXIDE"])
], dependencies: [.package(path: "../../shared/ide-theme")], targets: [
    .systemLibrary(name: "CLatexCore", path: "CLatexCore"),
    .target(name: "LaTeXIDEKit", dependencies: ["CLatexCore", .product(name: "IDEThemeAssets", package: "ide-theme")], linkerSettings: [
        .unsafeFlags([repository.appendingPathComponent("target/debug/liblatexide_ffi.a").path]),
        .linkedFramework("Security")
    ]),
    .executableTarget(name: "LaTeXIDE", dependencies: ["LaTeXIDEKit"]),
    .testTarget(name: "LaTeXIDEKitTests", dependencies: ["LaTeXIDEKit"])
])
