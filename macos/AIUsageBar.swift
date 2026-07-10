// AIUsageBar — native SwiftUI menu bar app for ai-usagebar.
//
// A macOS-native counterpart to the GNOME Shell extension and Waybar widget.
// It shows the 5-hour (session), weekly, optional model-specific, and optional
// extra-usage ($) windows in the menu bar with a native dropdown built from
// SwiftUI `Gauge`s — not Unicode text bars. Data still comes from the Rust
// `ai-usagebar` binary (same vendor/OAuth/Keychain logic), so this file is a
// thin, fully native UI over the proven engine.
//
// Build:  swiftc -O -parse-as-library AIUsageBar.swift -o ai-usagebar-menubar
//         (needs the Xcode command-line tools: `xcode-select --install`)
// Run:    ./ai-usagebar-menubar &      (or ./install-agent.sh for login start)
// macOS:  13+ (Ventura) — MenuBarExtra + Gauge are 13.0 APIs.
//
// First, on the Mac: run `claude` once so the OAuth creds land in the login
// Keychain — ai-usagebar reads them there (src/anthropic/keychain.rs).

import SwiftUI
import AppKit
import ServiceManagement

// ─── Settings (persisted in UserDefaults; edit in Preferences) ───────────
let DEF = UserDefaults.standard

let SETTINGS_DEFAULTS: [String: Any] = [
    "vendor": "anthropic",
    "interval": 30.0,
    "showSession": true,
    "showWeekly": true,
    "showSonnet": true,
    "showModelQuotas": true,
    "showExtra": false,
    "colorLow": "#98c379",
    "colorMid": "#e5c07b",
    "colorHigh": "#d19a66",
    "colorCritical": "#e06c75",
    "binaryPath": "",
]

var INTERVAL: Double { let v = DEF.double(forKey: "interval"); return v > 0 ? v : 30 }

// ─── Color helpers ───────────────────────────────────────────────────────
func nsHexColor(_ hex: String) -> NSColor {
    var s = hex
    if s.hasPrefix("#") { s.removeFirst() }
    guard s.count == 6, let v = UInt32(s, radix: 16) else { return .labelColor }
    return NSColor(srgbRed: CGFloat((v >> 16) & 0xff) / 255.0,
                   green: CGFloat((v >> 8) & 0xff) / 255.0,
                   blue: CGFloat(v & 0xff) / 255.0,
                   alpha: 1.0)
}

// Severity → Color, mirroring the GNOME extension thresholds
// (≥90 critical, ≥75 high, ≥50 mid, else low).
func severityColor(_ pct: Int, low: String, mid: String, high: String, critical: String) -> Color {
    let hex: String
    if pct >= 90 { hex = critical }
    else if pct >= 75 { hex = high }
    else if pct >= 50 { hex = mid }
    else { hex = low }
    return Color(nsColor: nsHexColor(hex))
}

// ─── Data model ──────────────────────────────────────────────────────────
struct Window: Equatable { let pct: Int; let reset: String }
struct Snapshot: Equatable {
    let vendor: String
    let plan: String
    let session: Window
    let weekly: Window
    let sonnet: Window?
    let fable: Window?
    let modelQuotas: [ModelQuota]
    let extra: Extra?
    let codeReview: Window?
    let creditBalance: String?
    struct ModelQuota: Equatable, Identifiable {
        var id: String { name }
        let name: String
        let window: Window
    }
    struct Extra: Equatable { let pct: Int; let spent: String; let limit: String }
}

func vendorDisplayName(_ vendor: String) -> String {
    switch vendor {
    case "anthropic": return "Claude"
    case "openai": return "Codex"
    case "zai": return "Z.AI"
    case "openrouter": return "OpenRouter"
    case "deepseek": return "DeepSeek"
    default: return vendor
    }
}

func stripMarkup(_ s: String) -> String {
    s.replacingOccurrences(of: "<[^>]*>", with: "", options: .regularExpression)
}

func countdown(_ iso: String?) -> String {
    guard let iso, !iso.isEmpty else { return "" }
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let date = formatter.date(from: iso) ?? ISO8601DateFormatter().date(from: iso)
    guard let date else { return "" }
    let seconds = max(0, Int(date.timeIntervalSinceNow))
    let days = seconds / 86400
    let hours = (seconds % 86400) / 3600
    let minutes = (seconds % 3600) / 60
    if days > 0 { return "\(days)d \(hours)h" }
    if hours > 0 { return "\(hours)h \(minutes)m" }
    return "\(minutes)m"
}

func parseJSON(_ text: String) -> Snapshot? {
    guard let data = text.data(using: .utf8),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let snap = obj["snapshot"] as? [String: Any] else { return nil }

    let vendor = obj["vendor"] as? String ?? "unknown"

    func int(_ value: Any?) -> Int? {
        if let i = value as? Int { return i }
        if let n = value as? NSNumber { return n.intValue }
        if let s = value as? String { return Int(s) }
        return nil
    }
    func window(_ dict: [String: Any]?) -> Window? {
        guard let dict else { return nil }
        return Window(pct: int(dict["utilization_pct"]) ?? 0,
                      reset: countdown(dict["resets_at"] as? String))
    }

    guard let session = window(snap["session"] as? [String: Any]),
          let weekly = window(snap["weekly"] as? [String: Any]) else { return nil }

    let modelQuotas = (snap["model_quotas"] as? [[String: Any]] ?? []).compactMap { quota -> Snapshot.ModelQuota? in
        guard let name = quota["name"] as? String,
              !name.trimmingCharacters(in: .whitespaces).isEmpty,
              let w = window(quota["window"] as? [String: Any]) else { return nil }
        return Snapshot.ModelQuota(name: name, window: w)
    }

    let extraDict = snap["extra"] as? [String: Any]
    let extra = extraDict.flatMap { e -> Snapshot.Extra? in
        guard let pct = int(e["utilization_pct"]) else { return nil }
        return Snapshot.Extra(pct: pct,
                              spent: e["spent"] as? String ?? "",
                              limit: e["limit"] as? String ?? "")
    }

    let creditsDict = snap["credits"] as? [String: Any]
    let creditBalance = creditsDict?["balance"] as? String
    let creditBalanceCleaned = creditBalance?.isEmpty == true ? nil : creditBalance

    return Snapshot(vendor: vendor,
                    plan: snap["plan"] as? String ?? "",
                    session: session,
                    weekly: weekly,
                    sonnet: window(snap["sonnet"] as? [String: Any]),
                    fable: window(snap["fable"] as? [String: Any]),
                    modelQuotas: modelQuotas,
                    extra: extra,
                    codeReview: window(snap["code_review"] as? [String: Any]),
                    creditBalance: creditBalanceCleaned)
}

// ─── Binary / subprocess helpers ─────────────────────────────────────────
func resolveBinary(_ name: String) -> String? {
    let fm = FileManager.default
    if name == "ai-usagebar" {
        let configured = DEF.string(forKey: "binaryPath") ?? ""
        if !configured.isEmpty, fm.isExecutableFile(atPath: configured) { return configured }
    }
    let home = NSHomeDirectory()
    for c in ["\(home)/.cargo/bin/\(name)", "/opt/homebrew/bin/\(name)", "/usr/local/bin/\(name)"]
    where fm.isExecutableFile(atPath: c) {
        return c
    }
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/usr/bin/which")
    p.arguments = [name]
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = FileHandle.nullDevice
    do {
        try p.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        let path = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if !path.isEmpty && fm.isExecutableFile(atPath: path) { return path }
    } catch {}
    return nil
}

// Run a command in Terminal.app (used for the TUI).
func openTuiInTerminal() {
    let tui = resolveBinary("ai-usagebar-tui") ?? "ai-usagebar-tui"
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    p.arguments = ["-e", "tell application \"Terminal\" to do script \"\(tui)\"",
                   "-e", "tell application \"Terminal\" to activate"]
    try? p.run()
}

// Native "Open at Login" via ServiceManagement (macOS 13+). Only works when
// running as a bundled `.app` (see bundle.sh); as a bare binary the register
// call throws, which we surface as a soft failure.
enum LoginItem {
    static var isEnabled: Bool { SMAppService.mainApp.status == .enabled }

    @discardableResult
    static func toggle() -> Bool {
        do {
            if isEnabled { try SMAppService.mainApp.unregister() }
            else { try SMAppService.mainApp.register() }
        } catch {
            NSSound.beep()   // e.g. running the un-bundled binary
        }
        return isEnabled
    }
}

// Open the native Settings scene from the menu-bar popover. The selector name
// changed across macOS versions; try the modern one, fall back to the legacy.
func openSettingsWindow() {
    NSApp.activate(ignoringOtherApps: true)
    if !NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil) {
        NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
    }
}

// ─── Observable usage model ──────────────────────────────────────────────
@MainActor
final class UsageModel: ObservableObject {
    static let shared = UsageModel()

    @Published var snapshots: [String: Snapshot] = [:]
    @Published var errorText: String?
    @Published var loading = false

    private var timer: Timer?
    private var started = false
    private let vendors = ["anthropic", "openai"]

    func start() {
        guard !started else { return }
        started = true
        refresh()
        restartTimer()
        NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification, object: nil, queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.restartTimer()
                self?.refresh()
            }
        }
    }

    func restartTimer() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: INTERVAL, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        }
    }

    func refresh() {
        guard let bin = resolveBinary("ai-usagebar") else {
            setError("ai-usagebar not found (PATH / ~/.cargo/bin / Homebrew)")
            return
        }
        loading = snapshots.isEmpty

        DispatchQueue.global(qos: .utility).async { [weak self] in
            var results: [String: Snapshot] = [:]
            var lastError: String?

            for vendor in self?.vendors ?? [] {
                let p = Process()
                p.executableURL = URL(fileURLWithPath: bin)
                p.arguments = ["--vendor", vendor, "--json"]
                let pipe = Pipe()
                p.standardOutput = pipe
                p.standardError = FileHandle.nullDevice
                var out = ""
                do {
                    try p.run()
                    let data = pipe.fileHandleForReading.readDataToEndOfFile()
                    p.waitUntilExit()
                    out = String(data: data, encoding: .utf8) ?? ""
                } catch {
                    lastError = "failed to run ai-usagebar for \(vendor)"
                    continue
                }
                if let snap = parseJSON(out) {
                    results[vendor] = snap
                } else {
                    let text = stripMarkup(out)
                    if !text.isEmpty { lastError = text }
                }
            }

            Task { @MainActor in
                guard let self else { return }
                self.loading = false
                if !results.isEmpty {
                    // A transient vendor failure must not make its previously
                    // displayed usage disappear from the menu bar.
                    self.snapshots.merge(results) { _, latest in latest }
                    self.errorText = nil
                } else if self.snapshots.isEmpty {
                    self.snapshots = [:]
                    self.errorText = lastError ?? "no data available"
                } else {
                    self.errorText = lastError
                }
            }
        }
    }

    private func setError(_ msg: String) {
        loading = false
        snapshots = [:]
        errorText = msg
    }
}

// ─── Menu-bar label (compact, native) ────────────────────────────────────
struct MenuBarLabel: View {
    @ObservedObject var model: UsageModel

    var body: some View {
        Group {
            if model.snapshots.isEmpty {
                HStack(spacing: 4) {
                    Image(systemName: "gauge.with.dots.needle.50percent")
                    Text(model.errorText != nil ? "⚠" : "…")
                }
            } else if entries.isEmpty {
                HStack(spacing: 4) {
                    Image(systemName: "gauge.with.dots.needle.50percent")
                    Text("idle")
                }
            } else {
                Image(nsImage: compactStatusImage)
                    .renderingMode(.template)
                    .accessibilityLabel(accessibilitySummary)
            }
        }
        .fixedSize()
    }

    private var entries: [(vendor: String, pct: Int)] {
        model.snapshots.keys.sorted().compactMap { vendor -> (vendor: String, pct: Int)? in
            entry(for: vendor)
        }
    }

    private func entry(for vendor: String) -> (vendor: String, pct: Int)? {
        guard let snapshot = model.snapshots[vendor], snapshot.session.pct >= 2 else { return nil }
        return (vendor, snapshot.session.pct)
    }

    private var compactStatusImage: NSImage {
        // MenuBarExtra drops later sibling views. Compose both marks and their
        // live values into one template image so AppKit sees one label while
        // still applying the native menu-bar tint in every state.
        let font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium)
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor.black,
        ]
        let markSize = CGFloat(17)
        let markGap = CGFloat(3)
        let separator = " · " as NSString
        let separatorSize = separator.size(withAttributes: attributes)
        let resources = ["anthropic": "ClaudeCodeMark", "openai": "CodexMark"]
        let segments = entries.map { entry in
            let text = "\(entry.pct)%" as NSString
            let image = resources[entry.vendor].flatMap {
                Bundle.main.url(forResource: $0, withExtension: "png")
            }.flatMap(NSImage.init(contentsOf:))
            return (image: image, text: text, textSize: text.size(withAttributes: attributes))
        }
        let width = segments.enumerated().reduce(CGFloat.zero) { total, item in
            let separatorWidth = item.offset == 0 ? 0 : separatorSize.width
            return total + separatorWidth + markSize + markGap + item.element.textSize.width
        }
        let height = max(markSize, ceil(segments.map(\.textSize.height).max() ?? markSize))
        let image = NSImage(size: NSSize(width: ceil(width), height: height), flipped: false) { _ in
            var x = CGFloat.zero
            for (index, segment) in segments.enumerated() {
                if index > 0 {
                    separator.draw(at: NSPoint(x: x, y: (height - separatorSize.height) / 2),
                                   withAttributes: attributes)
                    x += separatorSize.width
                }
                segment.image?.draw(in: NSRect(x: x, y: (height - markSize) / 2,
                                               width: markSize, height: markSize))
                x += markSize + markGap
                segment.text.draw(at: NSPoint(x: x, y: (height - segment.textSize.height) / 2),
                                  withAttributes: attributes)
                x += segment.textSize.width
            }
            return true
        }
        image.isTemplate = true
        return image
    }

    private var accessibilitySummary: String {
        entries.map { "\(vendorDisplayName($0.vendor)) \($0.pct) percent" }
            .joined(separator: ", ")
    }
}

// ─── Native gauge row ────────────────────────────────────────────────────
struct WindowRow: View {
    let name: String
    let pct: Int
    let value: String
    let reset: String?
    let tint: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(name).font(.system(size: 12, weight: .medium))
                Spacer()
                Text(value).font(.system(size: 12).monospacedDigit()).foregroundStyle(tint)
            }
            Gauge(value: Double(min(100, max(0, pct))) / 100.0) { EmptyView() }
                .gaugeStyle(.accessoryLinearCapacity)
                .tint(tint)
            if let r = reset, !r.isEmpty {
                Text("resets in \(r)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

// ─── Popover (dropdown) content ──────────────────────────────────────────
struct UsagePopover: View {
    @ObservedObject var model: UsageModel
    @AppStorage("showSession") private var showSession = true
    @AppStorage("showWeekly") private var showWeekly = true
    @AppStorage("showSonnet") private var showSonnet = true
    @AppStorage("showModelQuotas") private var showModelQuotas = true
    @AppStorage("showExtra") private var showExtra = false
    @AppStorage("colorLow") private var colorLow = "#98c379"
    @AppStorage("colorMid") private var colorMid = "#e5c07b"
    @AppStorage("colorHigh") private var colorHigh = "#d19a66"
    @AppStorage("colorCritical") private var colorCritical = "#e06c75"

    @State private var loginEnabled = false

    private func tint(_ pct: Int) -> Color {
        severityColor(pct, low: colorLow, mid: colorMid, high: colorHigh, critical: colorCritical)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            content
            Divider()
            actions
        }
        .padding(14)
        .frame(width: 300)
        .onAppear { loginEnabled = LoginItem.isEnabled }
    }

    @ViewBuilder private var content: some View {
        let vendors = model.snapshots.keys.sorted()
        if !vendors.isEmpty {
            ForEach(vendors, id: \.self) { key in
                if let s = model.snapshots[key] {
                    vendorSection(s)
                        .padding(.bottom, vendors.last == key ? 0 : 4)
                }
            }
        } else if let e = model.errorText {
            Label(e, systemImage: "exclamationmark.triangle.fill")
                .foregroundStyle(.red)
                .font(.system(size: 12))
                .fixedSize(horizontal: false, vertical: true)
        } else {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Loading…").foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder private func vendorSection(_ s: Snapshot) -> some View {
        Text(vendorDisplayName(s.vendor))
            .font(.headline)
        if showSession {
            WindowRow(name: "Session (5h)", pct: s.session.pct,
                      value: "\(s.session.pct)%", reset: s.session.reset, tint: tint(s.session.pct))
        }
        if showWeekly {
            WindowRow(name: "Weekly (7d)", pct: s.weekly.pct,
                      value: "\(s.weekly.pct)%", reset: s.weekly.reset, tint: tint(s.weekly.pct))
        }
        if showSonnet, let sn = s.sonnet {
            WindowRow(name: "Sonnet (7d)", pct: sn.pct,
                      value: "\(sn.pct)%", reset: sn.reset, tint: tint(sn.pct))
        }
        if showModelQuotas {
            ForEach(s.modelQuotas) { quota in
                WindowRow(name: "\(quota.name) (7d)", pct: quota.window.pct,
                          value: "\(quota.window.pct)%", reset: quota.window.reset,
                          tint: tint(quota.window.pct))
            }
        }
        if showExtra, let e = s.extra {
            WindowRow(name: "Extra usage", pct: e.pct,
                      value: "\(e.spent) / \(e.limit)", reset: nil, tint: tint(e.pct))
        }
        if let cr = s.codeReview {
            WindowRow(name: "Code Review (7d)", pct: cr.pct,
                      value: "\(cr.pct)%", reset: cr.reset, tint: tint(cr.pct))
        }
        if let bal = s.creditBalance {
            HStack {
                Text("Credits").font(.system(size: 12, weight: .medium))
                Spacer()
                Text(bal).font(.system(size: 12).monospacedDigit())
            }
        }
    }

    @ViewBuilder private var actions: some View {
        MenuButton(title: "Refresh now", systemImage: "arrow.clockwise") { model.refresh() }
        MenuButton(title: "Open TUI", systemImage: "terminal") { openTuiInTerminal() }
        MenuButton(title: "Open at Login",
                   systemImage: loginEnabled ? "checkmark.circle.fill" : "circle") {
            loginEnabled = LoginItem.toggle()
        }
        Divider()
        MenuButton(title: "Quit", systemImage: "power") { NSApp.terminate(nil) }
    }
}

// A borderless, full-width menu-style button row.
struct MenuButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// ─── Preferences (native SwiftUI Settings scene) ─────────────────────────
extension Color {
    init(hexString: String) { self.init(nsColor: nsHexColor(hexString)) }
    var hexString: String {
        let ns = NSColor(self).usingColorSpace(.sRGB) ?? .black
        return String(format: "#%02x%02x%02x",
                      Int((ns.redComponent * 255).rounded()),
                      Int((ns.greenComponent * 255).rounded()),
                      Int((ns.blueComponent * 255).rounded()))
    }
}

struct HexColorPicker: View {
    let title: String
    @Binding var hex: String
    var body: some View {
        ColorPicker(title, selection: Binding(
            get: { Color(hexString: hex) },
            set: { hex = $0.hexString }
        ), supportsOpacity: false)
    }
}

struct SettingsView: View {
    @AppStorage("interval") private var interval = 30.0
    @AppStorage("showSession") private var showSession = true
    @AppStorage("showWeekly") private var showWeekly = true
    @AppStorage("showSonnet") private var showSonnet = true
    @AppStorage("showModelQuotas") private var showModelQuotas = true
    @AppStorage("showExtra") private var showExtra = false
    @AppStorage("colorLow") private var colorLow = "#98c379"
    @AppStorage("colorMid") private var colorMid = "#e5c07b"
    @AppStorage("colorHigh") private var colorHigh = "#d19a66"
    @AppStorage("colorCritical") private var colorCritical = "#e06c75"
    @AppStorage("binaryPath") private var binaryPath = ""

    var body: some View {
        Form {
            Section("Display") {
                Toggle("Show session (5h)", isOn: $showSession)
                Toggle("Show weekly (7d)", isOn: $showWeekly)
                Toggle("Show Sonnet (7d)", isOn: $showSonnet)
                Toggle("Show model quotas (7d)", isOn: $showModelQuotas)
                Toggle("Show extra usage ($)", isOn: $showExtra)
            }
            Section("Severity colors") {
                HexColorPicker(title: "Low (<50%)", hex: $colorLow)
                HexColorPicker(title: "Mid (50–74%)", hex: $colorMid)
                HexColorPicker(title: "High (75–89%)", hex: $colorHigh)
                HexColorPicker(title: "Critical (≥90%)", hex: $colorCritical)
            }
            Section("Data") {
                Text("Fetching Claude + Codex usage")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Stepper("Refresh interval: \(Int(interval))s", value: $interval, in: 5...3600, step: 5)
                TextField("Binary path (empty = auto)", text: $binaryPath)
            }
        }
        .formStyle(.grouped)
        .frame(width: 420, height: 460)
    }
}

// ─── App entry point ─────────────────────────────────────────────────────
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)   // menu-bar agent, no Dock icon
        UsageModel.shared.start()               // fetch immediately, not on first click
    }
}

@main
struct AIUsageBarApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var model = UsageModel.shared

    init() { DEF.register(defaults: SETTINGS_DEFAULTS) }

    var body: some Scene {
        MenuBarExtra {
            UsagePopover(model: model)
        } label: {
            MenuBarLabel(model: model)
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsView()
        }
    }
}
