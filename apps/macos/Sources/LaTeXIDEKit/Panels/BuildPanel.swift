import SwiftUI
struct BuildPanel: View {
    let store: WorkspaceStore
    @Environment(\.ideTheme) private var theme
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 20) {
                tab(.problems, title: "Problémák", count: store.diagnostics.count)
                tab(.output, title: "Kimenet", count: nil)
                Spacer()
                if !store.buildState.isEmpty {
                    HStack(spacing: 5) { Circle().fill(theme.ui(store.building ? "accent" : store.buildState == "succeeded" ? "success" : "error")).frame(width: 5, height: 5); Text(store.buildLabel) }.font(.system(size: 10)).foregroundStyle(theme.ui("secondary"))
                }
                IDEIconButton(symbol: "xmark", label: "Alsó panel elrejtése", size: 25) { store.bottomPanel = nil }
            }.padding(.leading, 16).padding(.trailing, 6).frame(height: 34).background(theme.ui("sidebar"))
            Rectangle().fill(theme.ui("border")).frame(height: 1)
            if store.bottomPanel == .problems {
                if store.diagnostics.isEmpty {
                    HStack(spacing: 9) {
                        Image(systemName: store.buildState == "succeeded" ? "checkmark.circle" : "info.circle").foregroundStyle(theme.ui(store.buildState == "succeeded" ? "success" : "secondary"))
                        Text(store.buildState == "succeeded" ? "A fordító nem jelzett problémát." : "A fordítás diagnosztikái itt jelennek meg.").foregroundStyle(theme.ui("secondary"))
                        Spacer()
                    }.font(.system(size: 11)).padding(16).frame(maxHeight: .infinity, alignment: .top)
                } else {
                    ScrollView { LazyVStack(spacing: 0) { ForEach(store.diagnostics) { diagnostic in
                        Button {
                            if let file = diagnostic.file {
                                let relative = file.hasPrefix("./") ? String(file.dropFirst(2)) : file
                                store.navigate(file: relative, line: diagnostic.line ?? 1)
                            }
                        } label: {
                            HStack(alignment: .top, spacing: 8) {
                                Image(systemName: diagnostic.severity == "error" ? "xmark.circle.fill" : "exclamationmark.triangle.fill").foregroundStyle(theme.ui(diagnostic.severity == "error" ? "error" : "warning"))
                                Text(diagnostic.message).foregroundStyle(theme.ui("text")).frame(maxWidth: .infinity, alignment: .leading)
                                if let file = diagnostic.file { Text((file as NSString).lastPathComponent + (diagnostic.line.map { ":\($0)" } ?? "")).foregroundStyle(theme.ui("secondary")) }
                            }.font(.system(size: 11)).padding(.horizontal, 15).padding(.vertical, 8).contentShape(Rectangle())
                        }.buttonStyle(.plain).disabled(diagnostic.file == nil)
                    } } }
                }
            } else {
                ScrollView([.horizontal, .vertical]) {
                    Text(store.buildLog.isEmpty ? "A fordítás kimenete itt jelenik meg.  ⌘B" : store.buildLog)
                        .font(.system(size: 11, design: .monospaced)).textSelection(.enabled).foregroundStyle(theme.ui("secondary"))
                        .frame(maxWidth: .infinity, alignment: .topLeading).padding(14)
                }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        }.background(theme.ui("panel"))
    }
    private func tab(_ value: BottomTool, title: String, count: Int?) -> some View {
        Button { store.bottomPanel = value } label: {
            HStack(spacing: 6) { Text(title); if let count, count > 0 { Text(String(count)).font(.system(size: 9, weight: .semibold)).padding(.horizontal, 5).padding(.vertical, 1).background(Capsule().fill(theme.ui("selection"))) } }
                .font(.system(size: 11, weight: store.bottomPanel == value ? .medium : .regular)).foregroundStyle(theme.ui(store.bottomPanel == value ? "text" : "secondary"))
                .frame(height: 34).overlay(alignment: .bottom) { if store.bottomPanel == value { theme.ui("accent").frame(height: 2) } }
        }.buttonStyle(.plain)
    }
}
