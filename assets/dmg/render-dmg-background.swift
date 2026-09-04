import AppKit

// The picture behind the disk image window. It says one thing -- put the app in
// Applications -- and says it with an arrow, because the two icons Finder draws
// on top of it are the sentence and this is only the verb between them.
//
// The slots are empty on purpose. Finder puts Roamling.app over the left one and
// the Applications alias over the right one, and anything drawn underneath shows
// through the gaps in an icon and reads as a smudge.
//
// `scripts/build-dmg-background.sh` runs this and commits the result, for the
// same reason the app icon is committed: a build should not depend on which
// emoji font the machine happens to have.

// Kept in step with `scripts/build-dmg.sh`, which tells Finder where to put the
// icons. The two lists have to agree or the arrow points at nothing.
let canvas = NSSize(width: 640, height: 400)
let appSlot = CGPoint(x: 160, y: 185)      // Finder coordinates: y down from the top
let applicationsSlot = CGPoint(x: 480, y: 185)

// The same warm cream as the app icon, so the window a user meets first and the
// icon they end up with are recognisably one thing.
let top = NSColor(srgbRed: 1.00, green: 0.97, blue: 0.91, alpha: 1)
let bottom = NSColor(srgbRed: 0.99, green: 0.89, blue: 0.75, alpha: 1)
let ink = NSColor(srgbRed: 0.42, green: 0.31, blue: 0.20, alpha: 1)

/// Finder measures from the top, Cocoa draws from the bottom.
func flipped(_ point: CGPoint) -> CGPoint {
    CGPoint(x: point.x, y: canvas.height - point.y)
}

func render(scale: CGFloat) -> NSBitmapImageRep {
    let pixels = NSSize(width: canvas.width * scale, height: canvas.height * scale)
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: Int(pixels.width), pixelsHigh: Int(pixels.height),
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ) else { fatalError("could not make a \(Int(pixels.width))px canvas") }
    rep.size = canvas

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let context = NSGraphicsContext.current!.cgContext
    context.scaleBy(x: scale, y: scale)

    NSGradient(starting: top, ending: bottom)?
        .draw(in: NSRect(origin: .zero, size: canvas), angle: -90)

    let left = flipped(appSlot)
    let right = flipped(applicationsSlot)

    // The arrow runs between the two icons and stops well clear of both: an
    // arrowhead that touches the Applications icon looks like part of it.
    let iconHalfWidth: CGFloat = 56
    let clearance: CGFloat = 34
    let start = CGPoint(x: left.x + iconHalfWidth + clearance, y: left.y)
    let end = CGPoint(x: right.x - iconHalfWidth - clearance, y: right.y)
    let head: CGFloat = 26

    ink.withAlphaComponent(0.55).setStroke()
    let shaft = NSBezierPath()
    shaft.move(to: CGPoint(x: start.x, y: start.y))
    shaft.line(to: CGPoint(x: end.x - head * 0.6, y: end.y))
    shaft.lineWidth = 7
    shaft.lineCapStyle = .round
    shaft.stroke()

    ink.withAlphaComponent(0.55).setFill()
    let point = NSBezierPath()
    point.move(to: end)
    point.line(to: CGPoint(x: end.x - head, y: end.y + head * 0.62))
    point.line(to: CGPoint(x: end.x - head, y: end.y - head * 0.62))
    point.close()
    point.fill()

    // Under the arrow rather than under an icon, where Finder writes the names.
    let caption = NSAttributedString(
        string: "Drag Roamling into Applications",
        attributes: [
            .font: NSFont.systemFont(ofSize: 15, weight: .medium),
            .foregroundColor: ink.withAlphaComponent(0.62)
        ]
    )
    let size = caption.size()
    caption.draw(at: CGPoint(
        x: (canvas.width - size.width) / 2,
        y: flipped(CGPoint(x: 0, y: 300)).y
    ))

    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let directory = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "."
for (scale, name) in [(CGFloat(1), "dmg-background.png"), (CGFloat(2), "dmg-background@2x.png")] {
    guard let data = render(scale: scale).representation(using: .png, properties: [:]) else {
        fatalError("could not encode \(name)")
    }
    try! data.write(to: URL(fileURLWithPath: "\(directory)/\(name)"))
    print("wrote \(name)")
}
