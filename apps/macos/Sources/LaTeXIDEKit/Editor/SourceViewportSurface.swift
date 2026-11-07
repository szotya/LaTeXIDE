import AppKit

/// Synchronously paint native text into a viewport-sized, display-profile bitmap.
/// TextKit retains the complete source, shaping, selection and input ownership.
@MainActor final class SourceViewportSurface {
    let pixels = CALayer()
    private var context: CGContext?
    private var contextRect = NSRect.zero
    private var contextScale: CGFloat = 0
    private var contextColorSpace: CGColorSpace?
    private var background: NSColor?
    private var theme: ResolvedTheme?
    private var font: NSFont?
    private var damage: NSRect?
    private var invalid = true

    init() {
        pixels.name = "LaTeX source viewport"
        pixels.drawsAsynchronously = false
        pixels.contentsFormat = .RGBA8Uint
        pixels.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull()]
    }
    func invalidate() { invalid = true }
    func layout(in view: SourceTextView) {
        guard view.window != nil, let root = view.layer else { return }
        if pixels.superlayer !== root { root.insertSublayer(pixels, at: 0) }
        root.backgroundColor = view.backgroundColor.cgColor
        root.isOpaque = view.isOpaque; pixels.isOpaque = view.isOpaque
        var viewport = view.visibleRect.intersection(view.bounds)
        if let manager = view.layoutManager as? SourceLayoutManager,
           manager.hasStableSourceRow(), let font = view.font {
            let row = manager.lineFragmentRect(forGlyphAt: 0, effectiveRange: nil)
            let overhang = max(abs(font.boundingRectForFont.minY), abs(font.boundingRectForFont.maxY)) + 2
            let paint = NSRect(x: viewport.minX, y: view.textContainerOrigin.y + row.minY - overhang,
                               width: viewport.width, height: row.height + 2 * overhang)
            viewport = viewport.intersection(paint)
        }
        guard !viewport.isEmpty, !viewport.isNull, viewport.width.isFinite, viewport.height.isFinite else { return }
        // Avoid scaling the native glyph bitmap by a fractional backing pixel.
        viewport = view.convertFromBacking(view.convertToBacking(viewport).integral)
        pixels.frame = viewport
        pixels.contentsScale = view.window?.backingScaleFactor ?? 1
    }
    func invalidate(_ rect: NSRect, in view: SourceTextView) {
        let clipped = rect.intersection(pixels.frame)
        guard !clipped.isNull, !clipped.isEmpty else { return }
        // Repaint complete backing pixels at antialiased line/highlight edges.
        let dirty = view.convertFromBacking(view.convertToBacking(clipped).integral).intersection(pixels.frame)
        damage = damage.map { $0.union(dirty) } ?? dirty
    }
    func update(in view: SourceTextView, draw: (NSRect) -> Void) {
        guard let window = view.window else { return }
        let viewport = pixels.frame, scale = window.backingScaleFactor
        let width = ceil(viewport.width * scale), height = ceil(viewport.height * scale)
        guard width.isFinite, height.isFinite, width > 0, height > 0,
              width < CGFloat(Int.max), height < CGFloat(Int.max),
              let colorSpace = window.colorSpace?.cgColorSpace ?? CGColorSpace(name: CGColorSpace.sRGB) else { return }
        let fresh = context == nil || contextRect != viewport || contextScale != scale || contextColorSpace != colorSpace
        if fresh {
            guard let buffer = CGContext(data: nil, width: Int(width), height: Int(height), bitsPerComponent: 8,
                                         bytesPerRow: 0, space: colorSpace,
                                         bitmapInfo: CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue) else { return }
            buffer.scaleBy(x: scale, y: scale)
            buffer.translateBy(x: -viewport.minX, y: viewport.maxY)
            buffer.scaleBy(x: 1, y: -1)
            context = buffer; contextRect = viewport; contextScale = scale; contextColorSpace = colorSpace
        }
        let changedAppearance = background != view.backgroundColor || theme != view.editorTheme || font != view.font
        guard fresh || invalid || changedAppearance || damage != nil else { return }
        let repaint = fresh || invalid || changedAppearance ? viewport : (damage ?? viewport).intersection(viewport)
        damage = nil; invalid = false
        background = view.backgroundColor; theme = view.editorTheme; font = view.font
        guard let context, !repaint.isEmpty, !repaint.isNull else { return }
        context.saveGState(); context.clip(to: repaint)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: true)
        draw(repaint)
        NSGraphicsContext.restoreGraphicsState(); context.restoreGState()
        // CoreGraphics keeps earlier images immutable through copy-on-write.
        pixels.contents = context.makeImage()
    }
}
