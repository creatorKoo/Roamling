// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

import AppKit
import RoamlingShell

@MainActor
final class UsageGuideWindowController: NSWindowController, NSWindowDelegate {
    private let heading = NSTextField(labelWithString: "")
    private let text = NSTextView()
    private var acknowledge: (() -> Void)?

    init() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 550),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable],
                              backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.minSize = NSSize(width: 440, height: 360)
        super.init(window: window)
        window.delegate = self
        guard let content = window.contentView else { return }
        heading.frame = NSRect(x: 24, y: 498, width: 512, height: 32)
        heading.font = .systemFont(ofSize: 23, weight: .semibold)
        heading.autoresizingMask = [.width, .minYMargin]
        content.addSubview(heading)
        let scroll = NSScrollView(frame: NSRect(x: 24, y: 66, width: 512, height: 416))
        scroll.autoresizingMask = [.width, .height]
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        text.frame = scroll.contentView.bounds
        text.isEditable = false
        text.isSelectable = true
        text.drawsBackground = false
        text.font = .systemFont(ofSize: 15)
        text.textColor = .labelColor
        text.textContainerInset = NSSize(width: 0, height: 8)
        text.isVerticallyResizable = true
        text.isHorizontallyResizable = false
        text.autoresizingMask = [.width]
        text.maxSize = NSSize(width: .greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        text.textContainer?.containerSize = NSSize(width: scroll.contentSize.width,
                                                   height: .greatestFiniteMagnitude)
        text.textContainer?.widthTracksTextView = true
        scroll.documentView = text
        content.addSubview(scroll)
        let done = NSButton(title: localized("guide.done"), target: self, action: #selector(closeGuide))
        done.bezelStyle = .rounded
        done.keyEquivalent = "\r"
        done.frame = NSRect(x: 406, y: 20, width: 130, height: 32)
        done.autoresizingMask = [.minXMargin, .maxYMargin]
        content.addSubview(done)
        window.center()
    }

    required init?(coder: NSCoder) { return nil }

    func present(_ page: UsageGuide.Page, manual: Bool, acknowledge: @escaping () -> Void) {
        // Replace the contents on manual reopen, but retain a single window.
        self.acknowledge = acknowledge
        window?.title = page.title
        heading.stringValue = page.title
        text.string = page.body
        text.scrollToBeginningOfDocument(nil)
        if manual {
            window?.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
        } else {
            window?.orderFront(nil)
        }
    }

    func prepareForTermination() { acknowledge = nil }
    @objc private func closeGuide() { window?.performClose(nil) }
    func windowWillClose(_ notification: Notification) {
        let callback = acknowledge
        acknowledge = nil
        callback?()
    }
}
