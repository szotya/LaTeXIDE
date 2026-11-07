import SwiftUI
public struct WorkspaceView: View {
    let store: WorkspaceStore
    @ObservedObject var preferences: EditorPreferences
    @Environment(\.colorScheme) private var systemScheme
    @StateObject private var assistant = AssistantSession()
    public init(store: WorkspaceStore, preferences: EditorPreferences) { self.store = store; self.preferences = preferences }
    private var theme: ResolvedTheme { preferences.theme(systemDark: systemScheme == .dark) }
    public var body: some View {
        VStack(spacing: 0) {
            WorkspaceChrome(store: store, preferences: preferences)
            theme.ui("border").frame(height: 1)
            GeometryReader { geometry in
                let available = Double(geometry.size.width) - 47
                let side = min(preferences.sidebarWidth, max(190, available - 760))
                let content = available - (store.sidebar == nil ? 0 : side + 5)
                let preview = min(max(320, content * preferences.previewFraction), content - 385)
                let panel = min(preferences.outputHeight, max(95, Double(geometry.size.height) - 330))
                HStack(spacing: 0) {
                    ActivityRail(store: store)
                    theme.ui("border").frame(width: 1)
                    if store.sidebar != nil {
                        ProjectPanel(store: store).frame(width: side)
                        IDEResizeHandle(axis: .horizontal, label: "Projektpanel szélessége", value: $preferences.sidebarWidth, bounds: 190...max(190, min(420, available - 760)))
                    }
                    VStack(spacing: 0) {
                        HStack(spacing: 0) {
                            EditorWorkspace(store: store, theme: theme, fontSize: preferences.fontSize, completionEnabled: preferences.completionEnabled).frame(maxWidth: .infinity, maxHeight: .infinity)
                            if store.pdfVisible {
                                IDEResizeHandle(axis: .horizontal, label: "Előnézet és asszisztens szélessége", value: Binding(get: { preview }, set: { preferences.previewFraction = $0 / max(1, content) }), bounds: 320...max(320, content - 385), inverse: true)
                                InspectorPanel(store: store, session: assistant).frame(width: max(320, preview))
                            }
                        }.frame(maxHeight: .infinity)
                        if store.bottomPanel != nil {
                            IDEResizeHandle(axis: .vertical, label: "Kimenetpanel magassága", value: $preferences.outputHeight, bounds: 95...max(95, Double(geometry.size.height) - 330), inverse: true)
                            BuildPanel(store: store).frame(height: panel)
                        }
                    }
                }
            }
            WorkspaceStatusBar(store: store)
        }.background(theme.ui("editor")).environment(\.ideTheme, theme).tint(theme.ui("accent"))
            .onAppear { store.inspector = preferences.inspectorTab; configureAssistant() }
            .onDisappear { assistant.disconnect() }
            .onChange(of: store.inspector) { _, value in preferences.inspectorTab = value }
            .onChange(of: store.root) { _, _ in assistant.disconnect() }
            .onChange(of: store.sessionPath) { _, _ in configureAssistant() }
            .preferredColorScheme(preferences.appearance.colorScheme)
            .background(WindowChromeBridge(theme: theme, appearance: preferences.appearance))
            .overlay(alignment: .top) {
                if let mode = store.quickOpen {
                    ZStack(alignment: .top) {
                        Color.black.opacity(0.22).onTapGesture { store.quickOpen = nil; store.focusEditor() }
                        QuickOpen(store: store).id(mode).padding(.top, 60).environment(\.ideTheme, theme)
                    }.ignoresSafeArea(edges: .bottom)
                }
            }
            .alert("A művelet nem sikerült", isPresented: Binding(get: { store.failure != nil }, set: { if !$0 { store.failure = nil } })) {
                Button("Rendben") { store.failure = nil }
            } message: { Text(store.failure ?? "") }
            .frame(minWidth: 1080, minHeight: 680)
    }
    private func configureAssistant() {
        assistant.configure(root: store.root, sessionPath: store.sessionPath, bridgeURL: Bundle.main.resourceURL?.appendingPathComponent("latexide-mcp"))
    }
}
public struct AppearanceSettings: View {
    @ObservedObject var preferences: EditorPreferences
    @Environment(\.colorScheme) private var systemScheme
    public init(preferences: EditorPreferences) { self.preferences = preferences }
    public var body: some View {
        AppearancePicker(preferences: preferences).environment(\.ideTheme, preferences.theme(systemDark: systemScheme == .dark))
            .preferredColorScheme(preferences.appearance.colorScheme)
    }
}
