// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import CoreGraphics
import Foundation
import RoamlingPet

/// The placeholder cat's pixels. Moved out of `RoamlingPet` in W2 without a
/// line of the drawing changing: it is antialiased Bezier art, so no portable
/// blitter reproduces it, and every platform is free to draw its own.
enum MacPlaceholderArt {
    static func drawAtlas(columns: Int, rows: Int, cellWidth: Int, cellHeight: Int) -> CGImage? {
        let width = cellWidth * columns
        let height = cellHeight * rows
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: colorSpace,
            bitmapInfo: bitmapInfo.rawValue
        ) else { return nil }

        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        context.setShouldAntialias(true)

        let usedFrames = PlaceholderPetFactory.usedFrames
        for row in 0..<rows {
            for column in 0..<usedFrames[row] {
                context.saveGState()
                context.translateBy(
                    x: CGFloat(column * cellWidth),
                    y: CGFloat(height - (row + 1) * cellHeight)
                )
                drawCat(in: context, row: row, column: column)
                context.restoreGState()
            }
        }
        return context.makeImage()
    }

    private static func drawCat(in context: CGContext, row: Int, column: Int) {
        let phase = CGFloat(column % 4) / 4 * .pi * 2
        var lift = sin(phase) * 1.5
        if row == 1 || row == 2 { lift = CGFloat(column % 2) * 3 }
        if row == 4 {
            let jump: [CGFloat] = [0, 10, 28, 12, 0]
            lift += jump[min(column, jump.count - 1)]
        }
        if row == 5 { lift -= 7 }

        context.saveGState()
        context.translateBy(x: 0, y: lift)

        let outline = color(0.20, 0.16, 0.19, 1)
        let fur = color(0.95, 0.68, 0.48, 1)
        let cream = color(1.0, 0.91, 0.77, 1)
        let darkPatch = color(0.31, 0.27, 0.31, 1)
        let pink = color(0.94, 0.52, 0.58, 1)

        // Soft grounding shadow.
        context.setFillColor(color(0.08, 0.06, 0.08, row == 4 ? 0.08 : 0.16))
        context.fillEllipse(in: CGRect(x: 50, y: 23 - lift * 0.25, width: 92, height: 16))

        // Tail, behind the body.
        context.setStrokeColor(outline)
        context.setLineWidth(18)
        context.setLineCap(.round)
        context.move(to: CGPoint(x: 135, y: 68))
        let tailSwing = sin(phase) * 9
        context.addCurve(
            to: CGPoint(x: 157 + tailSwing, y: 102),
            control1: CGPoint(x: 160 + tailSwing, y: 57),
            control2: CGPoint(x: 172 + tailSwing, y: 90)
        )
        context.strokePath()
        context.setStrokeColor(fur)
        context.setLineWidth(12)
        context.move(to: CGPoint(x: 135, y: 68))
        context.addCurve(
            to: CGPoint(x: 157 + tailSwing, y: 102),
            control1: CGPoint(x: 160 + tailSwing, y: 57),
            control2: CGPoint(x: 172 + tailSwing, y: 90)
        )
        context.strokePath()

        // Body.
        let body = CGRect(x: 55, y: 38, width: 84, height: 91)
        fillAndStrokeEllipse(body, fill: fur, stroke: outline, lineWidth: 6, in: context)
        context.setFillColor(cream)
        context.fillEllipse(in: CGRect(x: 73, y: 46, width: 49, height: 66))

        // Ears. Failed poses flatten them slightly.
        let earTop = row == 5 ? CGFloat(132) : 166
        let leftEar = CGMutablePath()
        leftEar.move(to: CGPoint(x: 61, y: 135))
        leftEar.addLine(to: CGPoint(x: 69, y: earTop))
        leftEar.addLine(to: CGPoint(x: 91, y: 143))
        leftEar.closeSubpath()
        fillAndStroke(leftEar, fill: fur, stroke: outline, lineWidth: 6, in: context)
        let rightEar = CGMutablePath()
        rightEar.move(to: CGPoint(x: 105, y: 143))
        rightEar.addLine(to: CGPoint(x: 128, y: earTop))
        rightEar.addLine(to: CGPoint(x: 137, y: 135))
        rightEar.closeSubpath()
        fillAndStroke(rightEar, fill: fur, stroke: outline, lineWidth: 6, in: context)

        // Head and calico patches.
        fillAndStrokeEllipse(
            CGRect(x: 54, y: 101, width: 86, height: 68),
            fill: fur,
            stroke: outline,
            lineWidth: 6,
            in: context
        )
        context.setFillColor(cream)
        context.fillEllipse(in: CGRect(x: 62, y: 111, width: 42, height: 47))
        context.setFillColor(darkPatch)
        context.fillEllipse(in: CGRect(x: 104, y: 132, width: 27, height: 25))

        let lookOffset = gazeOffset(row: row, column: column)
        drawFace(
            in: context,
            row: row,
            column: column,
            lookOffset: lookOffset,
            outline: outline,
            cream: cream,
            pink: pink
        )

        // Feet and state-specific paw motion.
        context.setFillColor(cream)
        let stride = (row == 1 || row == 2) ? (column % 2 == 0 ? CGFloat(6) : -6) : 0
        context.fillEllipse(in: CGRect(x: 60 + stride, y: 30, width: 31, height: 20))
        context.fillEllipse(in: CGRect(x: 106 - stride, y: 30, width: 31, height: 20))

        if row == 3 || row == 6 {
            let wave = CGFloat(column % 4) * 4
            drawPaw(
                from: CGPoint(x: 126, y: 90),
                to: CGPoint(x: 151, y: 133 + wave),
                fur: fur,
                outline: outline,
                in: context
            )
        } else if row == 7 {
            let tap = column % 2 == 0 ? CGFloat(7) : 0
            drawPaw(
                from: CGPoint(x: 78, y: 75),
                to: CGPoint(x: 72, y: 49 + tap),
                fur: fur,
                outline: outline,
                in: context
            )
            drawPaw(
                from: CGPoint(x: 116, y: 75),
                to: CGPoint(x: 122, y: 56 - tap),
                fur: fur,
                outline: outline,
                in: context
            )
        }

        context.restoreGState()
    }

    private static func drawFace(
        in context: CGContext,
        row: Int,
        column: Int,
        lookOffset: CGPoint,
        outline: CGColor,
        cream: CGColor,
        pink: CGColor
    ) {
        if row == 5 {
            context.setStrokeColor(outline)
            context.setLineWidth(5)
            context.setLineCap(.round)
            context.move(to: CGPoint(x: 76, y: 132))
            context.addLine(to: CGPoint(x: 87, y: 128))
            context.move(to: CGPoint(x: 111, y: 128))
            context.addLine(to: CGPoint(x: 122, y: 132))
            context.strokePath()
        } else {
            context.setFillColor(cream)
            context.fillEllipse(in: CGRect(x: 72, y: 126, width: 22, height: 20))
            context.fillEllipse(in: CGRect(x: 105, y: 126, width: 22, height: 20))
            context.setFillColor(outline)
            context.fillEllipse(in: CGRect(x: 80 + lookOffset.x, y: 132 + lookOffset.y, width: 7, height: 9))
            context.fillEllipse(in: CGRect(x: 113 + lookOffset.x, y: 132 + lookOffset.y, width: 7, height: 9))
        }

        context.setFillColor(pink)
        let nose = CGMutablePath()
        nose.move(to: CGPoint(x: 95, y: 121))
        nose.addLine(to: CGPoint(x: 103, y: 121))
        nose.addLine(to: CGPoint(x: 99, y: 116))
        nose.closeSubpath()
        context.addPath(nose)
        context.fillPath()

        context.setStrokeColor(outline)
        context.setLineWidth(2.5)
        context.move(to: CGPoint(x: 99, y: 116))
        context.addCurve(
            to: CGPoint(x: 89, y: 113),
            control1: CGPoint(x: 97, y: 111),
            control2: CGPoint(x: 93, y: 111)
        )
        context.move(to: CGPoint(x: 99, y: 116))
        context.addCurve(
            to: CGPoint(x: 109, y: 113),
            control1: CGPoint(x: 101, y: 111),
            control2: CGPoint(x: 105, y: 111)
        )
        context.strokePath()

        if row == 8 {
            context.setStrokeColor(color(0.35, 0.65, 0.78, 0.85))
            context.setLineWidth(3)
            context.strokeEllipse(in: CGRect(x: 64, y: 119, width: 31, height: 29))
        }
    }

    private static func drawPaw(
        from start: CGPoint,
        to end: CGPoint,
        fur: CGColor,
        outline: CGColor,
        in context: CGContext
    ) {
        context.setLineCap(.round)
        context.setStrokeColor(outline)
        context.setLineWidth(20)
        context.move(to: start)
        context.addLine(to: end)
        context.strokePath()
        context.setStrokeColor(fur)
        context.setLineWidth(13)
        context.move(to: start)
        context.addLine(to: end)
        context.strokePath()
    }

    private static func gazeOffset(row: Int, column: Int) -> CGPoint {
        guard row == 9 || row == 10 else { return .zero }
        let index = row == 9 ? column : column + 8
        let radians = CGFloat(index) * 22.5 * .pi / 180
        return CGPoint(x: sin(radians) * 4, y: cos(radians) * 3)
    }

    private static func fillAndStrokeEllipse(
        _ rect: CGRect,
        fill: CGColor,
        stroke: CGColor,
        lineWidth: CGFloat,
        in context: CGContext
    ) {
        context.setFillColor(fill)
        context.fillEllipse(in: rect)
        context.setStrokeColor(stroke)
        context.setLineWidth(lineWidth)
        context.strokeEllipse(in: rect.insetBy(dx: lineWidth / 2, dy: lineWidth / 2))
    }

    private static func fillAndStroke(
        _ path: CGPath,
        fill: CGColor,
        stroke: CGColor,
        lineWidth: CGFloat,
        in context: CGContext
    ) {
        context.addPath(path)
        context.setFillColor(fill)
        context.fillPath()
        context.addPath(path)
        context.setStrokeColor(stroke)
        context.setLineWidth(lineWidth)
        context.setLineJoin(.round)
        context.strokePath()
    }

    private static func color(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat, _ alpha: CGFloat) -> CGColor {
        CGColor(red: red, green: green, blue: blue, alpha: alpha)
    }
}
