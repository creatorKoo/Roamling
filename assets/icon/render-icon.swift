import AppKit

// The mark is the same one the menu bar and the Windows tray show: 🐾, drawn
// rather than shipped, so there is one idea of what Roamling looks like.
// A Dock icon is not a bare glyph though -- macOS expects a rounded tile, and
// a transparent icon reads as broken next to everything else in the Dock.

// Apple's grid for a macOS app icon: the tile is 824 of 1024 with a corner
// radius of 185.4, centred, leaving the margin the Dock expects to see. Scaled
// per size below.

struct Variant {
    let name: String
    let top: NSColor
    let bottom: NSColor
}

// Warm cream, the same family as Mochi's own fur, so the pet and the app read
// as one thing. Deliberately quiet: an icon that shouts in the Dock is the
// same mistake as a pet that interrupts.
let variant = Variant(
    name: "cream",
    top: NSColor(srgbRed: 1.00, green: 0.97, blue: 0.91, alpha: 1),
    bottom: NSColor(srgbRed: 0.99, green: 0.89, blue: 0.75, alpha: 1)
)

/// The glyph's own ink, measured rather than guessed. Picking a font size means
/// guessing at side bearings, and guessing leaves the paws small in a box of
/// empty margin -- which is the note the Windows tray icon carries too.
func inkBounds(_ text: NSAttributedString, in canvas: NSSize) -> NSRect? {
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(canvas.width), pixelsHigh: Int(canvas.height),
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ) else { return nil }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    text.draw(at: .zero)
    NSGraphicsContext.restoreGraphicsState()

    var minX = Int.max, minY = Int.max, maxX = -1, maxY = -1
    for y in 0..<rep.pixelsHigh {
        for x in 0..<rep.pixelsWide {
            guard let colour = rep.colorAt(x: x, y: y), colour.alphaComponent > 0.02 else { continue }
            minX = min(minX, x); maxX = max(maxX, x)
            minY = min(minY, y); maxY = max(maxY, y)
        }
    }
    guard maxX >= minX else { return nil }
    // `colorAt` counts rows from the top; drawing counts from the bottom.
    return NSRect(
        x: CGFloat(minX), y: CGFloat(rep.pixelsHigh - 1 - maxY),
        width: CGFloat(maxX - minX + 1), height: CGFloat(maxY - minY + 1)
    )
}

let glyph = "\u{1F43E}"
let probeSize: CGFloat = 700
let probe = NSAttributedString(
    string: glyph,
    attributes: [.font: NSFont(name: "Apple Color Emoji", size: probeSize)!]
)
guard let ink = inkBounds(probe, in: NSSize(width: 1400, height: 1400)) else {
    print("could not measure the glyph"); exit(1)
}
// The paw fills a little over half the tile, which is where a simple mark sits
// comfortably against Apple's own icons.

let destination = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp/icon"

/// Every size an `.icns` carries. Each is drawn at its own size rather than
/// downscaled from 1024: the paw has small round toes, and a resampled 16px
/// version of them is mush where a drawn one is still a paw.
/// How much of the canvas the tile takes, and how much of the tile the mark
/// takes, at each size.
///
/// Apple's grid -- an 824 tile inside 1024, with the mark at a little over half
/// -- is right where there are pixels to spend. At 16 it is not: two paws at
/// 56% of an 824-of-1024 tile leaves each paw about four pixels across, which
/// is a smudge rather than a paw. So the margin shrinks and the mark grows as
/// the icon does not. Apple's own small icons do the same thing.
func grid(for pixels: Int) -> (margin: CGFloat, radius: CGFloat, fill: CGFloat) {
    switch pixels {
    case ...16: (14, 150, 0.86)
    case ...32: (40, 165, 0.76)
    case ...64: (70, 178, 0.64)
    default: (100, 185.4, 0.56)
    }
}

for pixels in [16, 32, 64, 128, 256, 512, 1024] {
    let side = CGFloat(pixels)
    let shape = grid(for: pixels)
    let tile = NSRect(x: side * shape.margin / 1024, y: side * shape.margin / 1024,
                      width: side * (1024 - shape.margin * 2) / 1024,
                      height: side * (1024 - shape.margin * 2) / 1024)
    let radius = side * shape.radius / 1024
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    )!
    let scale = tile.width * shape.fill / max(ink.width, ink.height)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

    let tilePath = NSBezierPath(roundedRect: tile, xRadius: radius, yRadius: radius)
    NSGradient(starting: variant.top, ending: variant.bottom)!
        .draw(in: tilePath, angle: -90)

    let context = NSGraphicsContext.current!.cgContext
    context.saveGState()
    let drawn = NSSize(width: ink.width * scale, height: ink.height * scale)
    context.translateBy(
        x: tile.midX - drawn.width / 2 - ink.minX * scale,
        y: tile.midY - drawn.height / 2 - ink.minY * scale
    )
    context.scaleBy(x: scale, y: scale)
    probe.draw(at: .zero)
    context.restoreGState()

    NSGraphicsContext.restoreGraphicsState()
    try! rep.representation(using: .png, properties: [:])!
        .write(to: URL(fileURLWithPath: "\(destination)/icon_\(pixels).png"))
}
