import SwiftUI
import AppKit
import LaTeXIDEKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var store: WorkspaceStore?
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        store?.requestTermination() == false ? .terminateCancel : .terminateNow
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
@main struct LaTeXIDEApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate
    @State private var store = WorkspaceStore()
    @StateObject private var preferences = EditorPreferences()
    var body: some Scene {
        Window("LaTeX IDE", id: "workspace") {
            WorkspaceView(store: store, preferences: preferences).onAppear {
                delegate.store = store
                let args = ProcessInfo.processInfo.arguments
                if let index = args.firstIndex(of: "--project"), index + 1 < args.count { store.open(URL(fileURLWithPath: args[index + 1])) }
            }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1440, height: 900)
        .commands {
            CommandGroup(replacing: .newItem) { Button("Projektmappa megnyitása…") { store.chooseProject() }.keyboardShortcut("o") }
            CommandGroup(replacing: .saveItem) { Button("Összes mentése") { store.save() }.keyboardShortcut("s") }
            CommandGroup(replacing: .undoRedo) {
                Button("Visszavonás") { store.undoCommand() }.keyboardShortcut("z")
                Button("Újra") { store.redoCommand() }.keyboardShortcut("z", modifiers: [.command, .shift])
            }
            CommandGroup(after: .saveItem) { Button("Fordítás") { store.build() }.keyboardShortcut("b") }
            CommandGroup(after: .textEditing) {
                Button("Keresés a fájlban") { store.findInEditor() }.keyboardShortcut("f")
                Button("Gyors fájlmegnyitás…") { store.showQuickOpen() }.keyboardShortcut("p")
                Button("Parancspaletta…") { store.showCommands() }.keyboardShortcut("p", modifiers: [.command, .shift])
            }
            CommandMenu("Munkaterület") {
                Button("Projektpanel") { store.toggleSidebar() }.keyboardShortcut("1")
                Button("Fordítási kimenet") { store.toggleOutput() }.keyboardShortcut("j")
                Button("PDF-előnézet") { store.togglePDF() }
                Button("Asszisztens") { store.showAssistant() }
                Divider()
                Button("Következő fájlfül") { store.adjacentTab(1) }.keyboardShortcut("]", modifiers: [.command, .shift])
                Button("Előző fájlfül") { store.adjacentTab(-1) }.keyboardShortcut("[", modifiers: [.command, .shift])
                Divider()
                Button("Paletta és megjelenés…") { store.showAppearance() }
            }
        }
        Settings { AppearanceSettings(preferences: preferences) }
    }
}
