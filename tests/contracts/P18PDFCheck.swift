// Offscreen validation only: no app/window/session is created.
import AppKit
import PDFKit

let args = CommandLine.arguments
precondition(args.count == 3, "PDF input and PNG output required")
let pdfURL = URL(fileURLWithPath: args[1])
guard let document = PDFDocument(url: pdfURL), document.pageCount == 1,
      let page = document.page(at: 0), let text = document.string,
      let cgDocument = CGPDFDocument(pdfURL as CFURL), let cgPage = cgDocument.page(at: 1) else {
    fatalError("Expected one parseable PDF page with extractable text")
}
let bounds = page.bounds(for: .mediaBox)
precondition(bounds.width > 0 && bounds.height > 0 && bounds.width <= 2000 && bounds.height <= 2000)
let width = Int(ceil(bounds.width)), height = Int(ceil(bounds.height))
guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                              bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
    fatalError("Could not create bounded PDF render")
}
context.setFillColor(CGColor(gray: 1, alpha: 1))
context.fill(CGRect(x: 0, y: 0, width: width, height: height))
context.translateBy(x: -bounds.minX, y: -bounds.minY)
context.drawPDFPage(cgPage)
guard let image = context.makeImage(), let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
    fatalError("Could not encode PDF render")
}
try png.write(to: URL(fileURLWithPath: args[2]), options: .withoutOverwriting)
let record: [String: Any] = ["pages": document.pageCount, "text": text]
print(String(data: try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys]), encoding: .utf8)!)
