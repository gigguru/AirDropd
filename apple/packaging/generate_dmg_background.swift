#!/usr/bin/swift
// Generates a professional drag-to-Applications DMG background (1x + 2x).

import AppKit
import Foundation

func usage() -> Never {
    fputs("Usage: generate_dmg_background.swift <logo.png> <out-1x.png> <out-2x.png>\n", stderr)
    exit(1)
}

let args = CommandLine.arguments
guard args.count == 4 else { usage() }

let logoPath = args[1]
let out1x = args[2]
let out2x = args[3]

guard let logo = NSImage(contentsOfFile: logoPath) else {
    fputs("Could not load logo: \(logoPath)\n", stderr)
    exit(1)
}

func drawBackground(size: NSSize, logo: NSImage) -> NSImage {
    let image = NSImage(size: size)
    image.lockFocus()

    let bounds = NSRect(origin: .zero, size: size)

    // Dark gradient wallpaper (modern macOS installer look).
    if let gradient = NSGradient(
        colors: [
            NSColor(calibratedRed: 0.10, green: 0.11, blue: 0.14, alpha: 1),
            NSColor(calibratedRed: 0.06, green: 0.07, blue: 0.10, alpha: 1),
        ]
    ) {
        gradient.draw(in: bounds, angle: 90)
    } else {
        NSColor(calibratedRed: 0.08, green: 0.09, blue: 0.12, alpha: 1).setFill()
        bounds.fill()
    }

    // Subtle top glow behind the logo.
    let glow = NSBezierPath(
        ovalIn: NSRect(
            x: size.width * 0.22,
            y: size.height * 0.52,
            width: size.width * 0.56,
            height: size.height * 0.38
        )
    )
    NSColor(calibratedWhite: 1, alpha: 0.04).setFill()
    glow.fill()

    // Logo centered above the drag zone.
    let logoSide = min(size.width, size.height) * 0.22
    let logoRect = NSRect(
        x: (size.width - logoSide) / 2,
        y: size.height * 0.58,
        width: logoSide,
        height: logoSide
    )
    logo.draw(in: logoRect, from: .zero, operation: .sourceOver, fraction: 1)

    // Title + instruction.
    let title = "AirDropd"
    let subtitle = "Drag to Applications to install"

    let titleFont = NSFont.systemFont(ofSize: size.height * 0.075, weight: .semibold)
    let subtitleFont = NSFont.systemFont(ofSize: size.height * 0.042, weight: .regular)

    let titleAttrs: [NSAttributedString.Key: Any] = [
        .font: titleFont,
        .foregroundColor: NSColor(calibratedWhite: 0.96, alpha: 1),
    ]
    let subtitleAttrs: [NSAttributedString.Key: Any] = [
        .font: subtitleFont,
        .foregroundColor: NSColor(calibratedWhite: 0.72, alpha: 1),
    ]

    let titleSize = (title as NSString).size(withAttributes: titleAttrs)
    let subtitleSize = (subtitle as NSString).size(withAttributes: subtitleAttrs)

    (title as NSString).draw(
        at: NSPoint(x: (size.width - titleSize.width) / 2, y: size.height * 0.46),
        withAttributes: titleAttrs
    )
    (subtitle as NSString).draw(
        at: NSPoint(x: (size.width - subtitleSize.width) / 2, y: size.height * 0.39),
        withAttributes: subtitleAttrs
    )

    // Decorative arrow hint between icon positions (660×400 layout coordinates scaled).
    let scale = size.width / 660
    let arrowY = 200 * scale
    let startX = 310 * scale
    let endX = 350 * scale
    let arrow = NSBezierPath()
    arrow.move(to: NSPoint(x: startX, y: arrowY))
    arrow.line(to: NSPoint(x: endX, y: arrowY))
    arrow.line(to: NSPoint(x: endX - 10 * scale, y: arrowY + 8 * scale))
    arrow.move(to: NSPoint(x: endX, y: arrowY))
    arrow.line(to: NSPoint(x: endX - 10 * scale, y: arrowY - 8 * scale))
    NSColor(calibratedWhite: 0.55, alpha: 0.85).setStroke()
    arrow.lineWidth = max(2, 2 * scale)
    arrow.stroke()

    image.unlockFocus()
    return image
}

func savePNG(_ image: NSImage, to path: String) throws {
    guard
        let tiff = image.tiffRepresentation,
        let rep = NSBitmapImageRep(data: tiff),
        let data = rep.representation(using: .png, properties: [:])
    else {
        throw NSError(domain: "generate_dmg_background", code: 1)
    }
    try data.write(to: URL(fileURLWithPath: path))
}

do {
    try savePNG(drawBackground(size: NSSize(width: 660, height: 400), logo: logo), to: out1x)
    try savePNG(drawBackground(size: NSSize(width: 1320, height: 800), logo: logo), to: out2x)
    print("Wrote \(out1x) and \(out2x)")
} catch {
    fputs("Failed to write PNG: \(error)\n", stderr)
    exit(1)
}
