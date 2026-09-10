import Cocoa
import WebKit
// render <url> <out.png> <w> <h> <light|dark> [scenario.js]
let a = CommandLine.arguments
let url = URL(string: a[1])!, out = a[2], w = Double(a[3])!, h = Double(a[4])!, dark = a[5] == "dark"
let scenario = a.count > 6 ? (try? String(contentsOfFile: a[6])) ?? "" : ""
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let win = NSWindow(contentRect: NSRect(x: 0, y: 0, width: w, height: h), styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
win.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
win.titlebarAppearsTransparent = true
win.titleVisibility = .hidden
let effect = NSVisualEffectView(frame: win.contentView!.bounds)
effect.material = .sidebar; effect.blendingMode = .behindWindow; effect.state = .active; effect.autoresizingMask = [.width, .height]
win.contentView = effect
let cfg = WKWebViewConfiguration()
let mock = try! String(contentsOfFile: URL(fileURLWithPath: a[0]).deletingLastPathComponent().appendingPathComponent("mock.js").path)
cfg.userContentController.addUserScript(WKUserScript(source: mock, injectionTime: .atDocumentStart, forMainFrameOnly: true))
let wv = WKWebView(frame: effect.bounds, configuration: cfg)
wv.autoresizingMask = [.width, .height]
wv.setValue(false, forKey: "drawsBackground")
effect.addSubview(wv)
app.activate(ignoringOtherApps: true)
win.makeKeyAndOrderFront(nil)
win.center()
wv.load(URLRequest(url: url))
var done = false
func snap() {
  let cfg = WKSnapshotConfiguration(); cfg.rect = wv.bounds
  wv.takeSnapshot(with: cfg) { img, err in
    guard let img = img else { print("snapshot failed: \(String(describing: err))"); done = true; return }
    // composite: vibrancy stand-in behind the transparent sidebar
    let size = img.size
    let final = NSImage(size: size)
    final.lockFocus()
    (dark ? NSColor(calibratedWhite: 0.16, alpha: 1) : NSColor(calibratedWhite: 0.93, alpha: 1)).setFill()
    NSRect(origin: .zero, size: size).fill()
    img.draw(at: .zero, from: NSRect(origin: .zero, size: size), operation: .sourceOver, fraction: 1)
    final.unlockFocus()
    let rep = NSBitmapImageRep(data: final.tiffRepresentation!)!
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
    print("wrote \(out)")
    done = true
  }
}
DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
  if scenario.isEmpty { snap(); return }
  wv.evaluateJavaScript("(async () => { \(scenario) })()") { _, e in
    if let e = e { print("scenario error: \(e)") }
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
      wv.evaluateJavaScript("String(window.__debug || '')") { r, _ in if let r = r as? String, !r.isEmpty { print("debug: \(r)") }; snap() }
    }
  }
}
while !done { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
