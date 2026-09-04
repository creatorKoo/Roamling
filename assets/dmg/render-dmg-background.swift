import AppKit

// The picture behind the disk image window. It says two things: drag the app
// across, and expect the first launch to be refused.
//
// One resolution, deliberately. Finder draws this a point per pixel from the
// top left of the window and does not scale it, so the image size is the
// layout, and a two-page hidpi TIFF only gave it a second size to disagree
// about. Softness on a window seen once is the cheaper problem.
//
// Everything sits in the top 340. The window height in the .DS_Store is the
// *frame* -- title bar included -- and Finder then takes another 30-odd points
// for the path bar if the reader has it switched on, which is a Finder-wide
// setting no disk image can override. Measured on a window asking for 400: 27
// points of title bar, 32 of path bar, 341 of content, and three lines of
// caption drawn below all of it. So the bottom of this picture is padding, and
// padding is what gets cut.
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
let appSlot = CGPoint(x: 160, y: 165)      // Finder coordinates: y down from the top
let applicationsSlot = CGPoint(x: 480, y: 165)

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

    // Below the icons, where Finder has stopped writing names.
    //
    // The second and third lines are the one thing a person cannot work out
    // from the window: the app is signed with a certificate Apple has never
    // seen, so the first launch is refused outright and the only way through is
    // a switch in System Settings. Without this the app looks broken, which is
    // a worse first impression than an extra line of text.
    func caption(_ text: String, size points: CGFloat, alpha: CGFloat, atY y: CGFloat) {
        let line = NSAttributedString(
            string: text,
            attributes: [
                .font: NSFont.systemFont(ofSize: points, weight: points > 13 ? .medium : .regular),
                .foregroundColor: ink.withAlphaComponent(alpha)
            ]
        )
        let measured = line.size()
        line.draw(at: CGPoint(
            x: (canvas.width - measured.width) / 2,
            y: flipped(CGPoint(x: 0, y: y)).y - measured.height
        ))
    }
    caption("Drag Roamling into Applications", size: 15, alpha: 0.62, atY: 270)
    caption(
        "First launch is refused \u{2014} System Settings \u{203A} Privacy & Security \u{203A} Open Anyway",
        size: 11.5, alpha: 0.44, atY: 298
    )
    caption(
        "첫 실행은 차단됩니다 \u{2014} 시스템 설정 \u{203A} 개인정보 보호 및 보안 \u{203A} 그래도 열기",
        size: 11.5, alpha: 0.44, atY: 316
    )

    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let directory = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "."
guard let data = render(scale: 1).representation(using: .png, properties: [:]) else {
    fatalError("could not encode the background")
}
try! data.write(to: URL(fileURLWithPath: "\(directory)/dmg-background.png"))
print("wrote dmg-background.png")
