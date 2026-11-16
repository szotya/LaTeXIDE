import AppKit
import SwiftUI

/// The invisible workspace attachment owns exactly one native titlebar accessory.
/// The workspace keeps the document, preferences, and assistant session alive.
final class TitlebarToolbarAttachment: NSView {
    private var controller: TitlebarToolbarController?
    private var chrome: AnyView
    private var theme: ResolvedTheme
    private var appearanceMode: AppearanceMode

    init(store: WorkspaceStore, preferences: EditorPreferences, theme: ResolvedTheme, appearance: AppearanceMode) {
        chrome = Self.content(store: store, preferences: preferences, theme: theme)
        self.theme = theme; appearanceMode = appearance
        super.init(frame: .zero)
    }
    required init?(coder: NSCoder) { nil }
    override var mouseDownCanMoveWindow: Bool { false }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if controller?.window !== window { detach() }
        guard let window else { return }
        if controller == nil { controller = TitlebarToolbarController(window: window, chrome: chrome) }
        updateWindow()
    }
    func update(store: WorkspaceStore, preferences: EditorPreferences, theme: ResolvedTheme, appearance: AppearanceMode) {
        chrome = Self.content(store: store, preferences: preferences, theme: theme)
        self.theme = theme; appearanceMode = appearance
        controller?.hosting.rootView = chrome
        updateWindow()
    }
    private static func content(store: WorkspaceStore, preferences: EditorPreferences, theme: ResolvedTheme) -> AnyView {
        AnyView(WorkspaceChrome(store: store, preferences: preferences).environment(\.ideTheme, theme))
    }
    private func updateWindow() {
        guard let window else { return }
        window.backgroundColor = theme.ns("chrome")
        window.appearance = appearanceMode == .system ? nil : NSAppearance(named: theme.dark ? .darkAqua : .aqua)
        controller?.layout()
    }
    func detach() { controller?.detach(); controller = nil }
}

/// AppKit supplies the traffic-light geometry and the titlebar's vertical extent.
/// A right accessory shares that row; there is no content-area toolbar or guessed
/// traffic-light padding. No private titlebar classes or replacement window buttons.
@MainActor final class TitlebarToolbarController {
    private(set) weak var window: NSWindow?
    let hosting: TitlebarHostingView
    let accessory = NSTitlebarAccessoryViewController()
    private let surface = TitlebarAccessorySurface()
    private let toolbar = NSToolbar(identifier: "hu.latexide.workspace-titlebar")
    private let previousToolbar: NSToolbar?
    private let previousStyle: NSWindow.ToolbarStyle
    private let previousBackgroundDragging: Bool
    private var observations: [NSObjectProtocol] = []
    private var layingOut = false

    init(window: NSWindow, chrome: AnyView) {
        self.window = window
        previousToolbar = window.toolbar; previousStyle = window.toolbarStyle
        previousBackgroundDragging = window.isMovableByWindowBackground
        hosting = TitlebarHostingView(rootView: chrome)
        hosting.sizingOptions = []
        hosting.setAccessibilityIdentifier("workspace-titlebar-controls")
        surface.addSubview(hosting)
        surface.frame = NSRect(x: 0, y: 0, width: 1, height: 38)
        surface.layoutControls = { [weak self] in self?.layout() }
        accessory.layoutAttribute = .right; accessory.view = surface
        toolbar.allowsUserCustomization = false; toolbar.autosavesConfiguration = false
        toolbar.showsBaselineSeparator = true
        window.titleVisibility = .hidden; window.titlebarAppearsTransparent = true
        window.isMovableByWindowBackground = false
        window.toolbarStyle = .unifiedCompact; window.toolbar = toolbar
        window.addTitlebarAccessoryViewController(accessory)
        for name in [NSWindow.didResizeNotification, NSWindow.didChangeBackingPropertiesNotification,
                     NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification] {
            observations.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.layout() } })
        }
        layout()
    }
    func layout() {
        guard !layingOut, let window, let frameView = window.contentView?.superview else { return }
        layingOut = true; defer { layingOut = false }
        let types: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
        let buttons = types.compactMap { window.standardWindowButton($0) }
        let frames = buttons.filter { !$0.isHidden }.map { $0.convert($0.bounds, to: frameView) }
        // Full screen can hide the native buttons. Its content layout remains the
        // fallback authority; never reserve a fixed traffic-light width.
        let nativeRight = frames.map(\.maxX).max() ?? window.contentLayoutRect.minX
        let width = max(0, frameView.bounds.width - nativeRight - 12)
        if abs(surface.frame.width - width) > 0.5 { surface.setFrameSize(NSSize(width: width, height: surface.frame.height)) }
        let center = buttons.first(where: { !$0.isHidden }).map { $0.convert($0.bounds, to: surface).midY } ?? surface.bounds.midY
        hosting.frame = NSRect(x: 0, y: center - 14, width: max(0, surface.bounds.width - 12), height: 28)
    }
    func detach() {
        observations.forEach(NotificationCenter.default.removeObserver); observations.removeAll()
        guard let window else { return }
        if let index = window.titlebarAccessoryViewControllers.firstIndex(where: { $0 === accessory }) {
            window.removeTitlebarAccessoryViewController(at: index)
        }
        if window.toolbar === toolbar {
            window.toolbar = previousToolbar; window.toolbarStyle = previousStyle
            window.isMovableByWindowBackground = previousBackgroundDragging
        }
        self.window = nil
    }
    deinit { MainActor.assumeIsolated { detach() } }
}

final class TitlebarHostingView: NSHostingView<AnyView> {
    override var mouseDownCanMoveWindow: Bool { false }
}
private final class TitlebarAccessorySurface: NSView {
    var layoutControls: (() -> Void)?
    override var mouseDownCanMoveWindow: Bool { false }
    override func layout() { super.layout(); layoutControls?() }
}

/// Only the two explicit empty gaps between control groups initiate dragging.
struct TitlebarDragRegion: NSViewRepresentable {
    func makeNSView(context: Context) -> TitlebarDragView { TitlebarDragView() }
    func updateNSView(_ view: TitlebarDragView, context: Context) {}
}
final class TitlebarDragView: NSView {
    var doubleClickPreference: () -> String? = {
        UserDefaults.standard.string(forKey: "AppleActionOnDoubleClick")
    }
    override var mouseDownCanMoveWindow: Bool { true }
    override func mouseDown(with event: NSEvent) {
        guard let window else { return }
        if event.clickCount == 2 {
            switch doubleClickPreference() {
            case "Maximize": window.performZoom(nil); return
            case "Minimize": window.performMiniaturize(nil); return
            default: break
            }
        }
        window.performDrag(with: event)
    }
}


/// SwiftUI owns each callback and label. AppKit owns the actual titlebar button,
/// native hit region, enablement, responder behavior, and target/action delivery.
struct TitlebarActionButton<Label: View>: NSViewRepresentable {
    let id: String
    let title: String
    let action: () -> Void
    let usesIntrinsicWidth: Bool
    let label: Label
    @Environment(\.isEnabled) private var enabled
    @Environment(\.ideTheme) private var theme

    init(id: String, title: String, action: @escaping () -> Void, usesIntrinsicWidth: Bool = false, @ViewBuilder label: () -> Label) {
        self.id = id; self.title = title; self.action = action
        self.usesIntrinsicWidth = usesIntrinsicWidth; self.label = label()
    }
    private var hostedLabel: AnyView { AnyView(label.environment(\.ideTheme, theme).environment(\.isEnabled, enabled)) }
    func makeCoordinator() -> TitlebarButtonActionCoordinator { TitlebarButtonActionCoordinator(action: action) }
    func makeNSView(context: Context) -> TitlebarNativeButton {
        let button = TitlebarNativeButton(label: hostedLabel, usesIntrinsicWidth: usesIntrinsicWidth)
        button.target = context.coordinator; button.action = #selector(TitlebarButtonActionCoordinator.invoke(_:))
        return button
    }
    func updateNSView(_ button: TitlebarNativeButton, context: Context) {
        context.coordinator.action = action
        button.setAccessibilityIdentifier(id); button.setAccessibilityLabel(title)
        button.toolTip = title; button.isEnabled = enabled
        button.usesIntrinsicWidth = usesIntrinsicWidth
        button.labelHost.rootView = hostedLabel
        button.labelHost.invalidateIntrinsicContentSize()
        button.invalidateIntrinsicContentSize(); button.needsLayout = true
    }
    func sizeThatFits(_ proposal: ProposedViewSize, nsView: TitlebarNativeButton, context: Context) -> CGSize? {
        nsView.sizeThatFits(proposal)
    }
}
@MainActor final class TitlebarButtonActionCoordinator: NSObject {
    var action: () -> Void
    init(action: @escaping () -> Void) { self.action = action }
    @objc func invoke(_ sender: NSButton) { action() }
}

final class TitlebarNativeButton: NSButton {
    let labelHost: TitlebarButtonLabel
    var usesIntrinsicWidth: Bool
    init(label: AnyView, usesIntrinsicWidth: Bool) {
        labelHost = TitlebarButtonLabel(rootView: label)
        self.usesIntrinsicWidth = usesIntrinsicWidth
        super.init(frame: .zero)
        title = ""; isBordered = false; bezelStyle = .regularSquare
        // No min/max constraints: expose the label's ideal size to the button.
        // With sizingOptions=[], NSView.fittingSize only sees an empty Auto
        // Layout system and can return zero even while the label draws wider.
        labelHost.sizingOptions = [.intrinsicContentSize]
        labelHost.setAccessibilityElement(false)
        addSubview(labelHost)
    }
    required init?(coder: NSCoder) { nil }
    override var mouseDownCanMoveWindow: Bool { false }
    override func layout() { super.layout(); labelHost.frame = bounds }
    override var intrinsicContentSize: NSSize {
        NSSize(width: max(0, labelHost.intrinsicContentSize.width.rounded(.up)), height: 28)
    }
    func sizeThatFits(_ proposal: ProposedViewSize) -> CGSize {
        let ideal = intrinsicContentSize
        // Build/Stop is content-sized, including when SwiftUI probes minimum
        // width with zero. Flexible project/search/icon controls keep their
        // parent layout proposals; no width is inferred from AX or a screenshot.
        let offered = proposal.width ?? ideal.width
        let width = usesIntrinsicWidth ? ideal.width : offered.isFinite ? max(0, offered) : ideal.width
        return CGSize(width: width, height: ideal.height)
    }
    override func accessibilityPerformPress() -> Bool {
        guard isEnabled, let action, target != nil else { return false }
        return sendAction(action, to: target)
    }
}
final class TitlebarButtonLabel: NSHostingView<AnyView> {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var mouseDownCanMoveWindow: Bool { false }
}

struct TitlebarIconButton: View {
    let id: String
    let symbol: String
    let label: String
    var selected = false
    let action: () -> Void
    @Environment(\.ideTheme) private var theme
    @Environment(\.isEnabled) private var enabled
    @State private var hover = false
    var body: some View {
        TitlebarActionButton(id: id, title: label, action: action) {
            Image(systemName: symbol).font(.system(size: 12, weight: .medium))
                .foregroundStyle(theme.ui(selected ? "accent" : "secondary"))
                .frame(width: 28, height: 28)
                .background(RoundedRectangle(cornerRadius: 5).fill(selected ? theme.ui("accent").opacity(0.14) : hover ? theme.ui("hover") : .clear))
                .opacity(enabled ? 1 : 0.35)
        }.frame(width: 28, height: 28).onHover { hover = $0 }.help(label)
    }
}

struct TitlebarMainDocumentPicker: NSViewRepresentable {
    let files: [String]
    @Binding var selection: String
    @Environment(\.isEnabled) private var enabled
    @Environment(\.ideTheme) private var theme
    func makeCoordinator() -> Coordinator { Coordinator(selection: $selection) }
    func makeNSView(context: Context) -> NSPopUpButton {
        let button = NSPopUpButton(frame: .zero, pullsDown: true)
        button.isBordered = false; button.controlSize = .small
        button.font = .systemFont(ofSize: 11)
        button.cell?.lineBreakMode = .byTruncatingMiddle
        button.target = context.coordinator; button.action = #selector(Coordinator.selectFile(_:))
        button.setAccessibilityIdentifier("workspace-main-document")
        return button
    }
    func updateNSView(_ button: NSPopUpButton, context: Context) {
        context.coordinator.selection = $selection
        button.isEnabled = enabled; button.contentTintColor = theme.ns("secondary")
        button.setAccessibilityLabel((selection as NSString).lastPathComponent)
        let menu = NSMenu(); menu.autoenablesItems = false
        let label = NSMenuItem(title: (selection as NSString).lastPathComponent, action: nil, keyEquivalent: "")
        label.image = NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
        menu.addItem(label)
        let heading = NSMenuItem(title: "Fődokumentum", action: nil, keyEquivalent: ""); heading.isEnabled = false
        menu.addItem(heading); menu.addItem(.separator())
        for file in files {
            let item = NSMenuItem(title: file, action: nil, keyEquivalent: "")
            item.representedObject = file; item.state = file == selection ? .on : .off
            menu.addItem(item)
        }
        button.menu = menu
    }
    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSPopUpButton, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? min(155, nsView.intrinsicContentSize.width), height: 28)
    }
    @MainActor final class Coordinator: NSObject {
        var selection: Binding<String>
        init(selection: Binding<String>) { self.selection = selection }
        @objc func selectFile(_ sender: NSPopUpButton) {
            guard let file = sender.selectedItem?.representedObject as? String else { return }
            selection.wrappedValue = file
        }
    }
}
