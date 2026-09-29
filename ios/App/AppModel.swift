import AVFoundation
import SwiftUI
import UIKit

struct PeerInfo: Decodable, Hashable {
    var id: String
    var name: String
    var addr: String
    var receiving: Bool
    var ip: String { String(addr.split(separator: ":").first ?? "") }
}

struct StreamInfo: Decodable, Hashable {
    var id: UInt32
    var name: String
    var from: String
    var bufferMs: Double
    var targetMs: Double
    var captureMs: Double
    var lost: UInt64
    var late: UInt64
    var underruns: UInt64
    var level: Float
    var rate: UInt32
}

struct EngineState: Decodable {
    var id = ""
    var name = ""
    var ips: [String] = []
    var mode = "balanced"
    var outputMs = 0.0
    var captureMs = 0.0
    var receiving = false
    var receiverError = ""
    var playLocal = true
    var forwarded: UInt64 = 0
    var sending = false
    var sentPackets: UInt64 = 0
    var sendLevel: Float = 0
    var sendError = ""
    var peers: [PeerInfo] = []
    var streams: [StreamInfo] = []
}

enum AppTab: Hashable { case send, receive, settings }
enum SendSource: String { case screen, mic }
enum VizStyle: String { case bars, wave }

/// A device that can be ticked in a list: found on the network, or added by IP.
struct DeviceItem: Identifiable, Hashable {
    var ip: String
    var name: String
    var dest: String
    var receiving: Bool
    var manual: Bool
    var id: String { ip }
}

/// Everything the screens show and do. Owns the engine and the audio.
@MainActor
final class AppModel: ObservableObject {
    let engine: Ssnd
    let audio: AudioIO
    private let link = LoopLink(listen: LoopLink.appPort)
    private let d = UserDefaults.standard
    private var timer: Timer?
    private var ticks = 0
    private var lastConfigSent = ""
    private var lastConfigAt = Date.distantPast
    private var lastReportAt = Date.distantPast
    private var stutterSeen: [UInt32: (count: UInt64, at: Date)] = [:]

    @Published var state = EngineState()
    @Published var tab: AppTab = .receive
    @Published var toast: String? = nil
    private var toastTask: Task<Void, Never>?

    // Saved settings.
    @Published var volume: Double { didSet { d.set(volume, forKey: "volume"); applyVolume() } }
    @Published var muted: Bool { didSet { d.set(muted, forKey: "muted"); applyVolume() } }
    @Published var mode: String { didSet { d.set(mode, forKey: "mode"); engine.setMode(mode); pushConfig(force: true) } }
    @Published var sendTargets: Set<String> { didSet { d.set(Array(sendTargets), forKey: "sendTargets"); applyTargets() } }
    @Published var forwardTargets: Set<String> { didSet { d.set(Array(forwardTargets), forKey: "forwardTargets"); applyForward() } }
    @Published var manualIPs: [String] { didSet { d.set(manualIPs, forKey: "manualIPs"); engine.setManualPeers(manualIPs); applyTargets(); applyForward() } }
    @Published var vizStyle: VizStyle { didSet { d.set(vizStyle.rawValue, forKey: "vizStyle") } }
    @Published var keepAwake: Bool { didSet { d.set(keepAwake, forKey: "keepAwake"); applyIdle() } }
    @Published var autoReceive: Bool { didSet { d.set(autoReceive, forKey: "autoReceive") } }
    @Published var sendSource: SendSource { didSet { d.set(sendSource.rawValue, forKey: "sendSource") } }
    @Published var playLocal: Bool { didSet { engine.setPlayLocal(playLocal) } }

    @Published private(set) var micSending = false
    @Published private(set) var report: BroadcastReport? = nil
    @Published var updateText = ""
    @Published var newer: Updater.Release? = nil
    @Published var allowBeta: Bool { didSet { d.set(allowBeta, forKey: "allowBeta"); Task { await checkUpdate(quiet: true) } } }
    @Published var checkingUpdate = false
    private var active = true

    init() {
        let d = UserDefaults.standard
        let name = d.string(forKey: "name") ?? UIDevice.current.name
        let mode = d.string(forKey: "mode") ?? "balanced"
        engine = Ssnd(name: name, mode: mode, announce: true) ?? Ssnd(name: "iPhone", mode: "balanced", announce: false)!
        audio = AudioIO(engine: engine)
        volume = d.object(forKey: "volume") as? Double ?? 1.0
        muted = d.bool(forKey: "muted")
        self.mode = mode
        sendTargets = Set(d.stringArray(forKey: "sendTargets") ?? [])
        forwardTargets = Set(d.stringArray(forKey: "forwardTargets") ?? [])
        manualIPs = d.stringArray(forKey: "manualIPs") ?? []
        vizStyle = VizStyle(rawValue: d.string(forKey: "vizStyle") ?? "") ?? .bars
        keepAwake = d.object(forKey: "keepAwake") as? Bool ?? true
        autoReceive = d.object(forKey: "autoReceive") as? Bool ?? true
        sendSource = SendSource(rawValue: d.string(forKey: "sendSource") ?? "") ?? .screen
        playLocal = true
        allowBeta = d.object(forKey: "allowBeta") as? Bool ?? true

        engine.setManualPeers(manualIPs)
        applyVolume()
        applyForward()
        audio.onProblem = { [weak self] msg in self?.show(msg) }
        if autoReceive { setReceiving(true) }
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
        Task { await checkUpdate(quiet: true) }
    }

    /// Update checks: at launch, then every 5 minutes while the app is open.
    private var lastUpdateCheck = Date()
    private static let updateEvery: TimeInterval = 5 * 60

    private func maybeCheckUpdate() {
        guard active, !checkingUpdate, Date().timeIntervalSince(lastUpdateCheck) >= AppModel.updateEvery else { return }
        Task { await checkUpdate(quiet: true) }
    }

    // MARK: polling

    private func tick() {
        ticks += 1
        if let data = link?.receive(), let r = try? JSONDecoder().decode(BroadcastReport.self, from: data) {
            if r.note == "stopped" {
                report = nil
                lastReportAt = .distantPast
            } else {
                report = r
                lastReportAt = Date()
            }
        } else if report != nil, Date().timeIntervalSince(lastReportAt) > 1.5 {
            report = nil
        }
        // 10 times a second in front, once a second in the background.
        if ticks % (active ? 2 : 20) == 0 {
            refresh()
        }
        if ticks % 20 == 0 {
            pushConfig(force: false)
            maybeCheckUpdate()
        }
    }

    func refresh() {
        guard let s = try? JSONDecoder().decode(EngineState.self, from: engine.stateJSON()) else { return }
        let now = Date()
        for st in s.streams {
            if let seen = stutterSeen[st.id] {
                if st.underruns > seen.count { stutterSeen[st.id] = (st.underruns, now) }
            } else {
                stutterSeen[st.id] = (st.underruns, .distantPast)
            }
        }
        state = s
        applyIdle()
    }

    func setActive(_ on: Bool) {
        active = on
        if on {
            refresh()
            pushConfig(force: true)
        }
    }

    // MARK: toast

    func show(_ msg: String) {
        toast = msg
        toastTask?.cancel()
        toastTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            if !Task.isCancelled { self?.toast = nil }
        }
    }

    func copy(_ text: String) {
        UIPasteboard.general.string = text
        show("คัดลอก \(text) แล้ว")
    }

    // MARK: receiving

    var receiving: Bool { state.receiving }

    func setReceiving(_ on: Bool) {
        if on {
            if let err = engine.startReceiving() {
                show("เปิดรับเสียงไม่ได้: \(err)")
                return
            }
        } else {
            engine.stopReceiving()
        }
        if let err = audio.set(play: on, mic: micSending) { show(err) }
        refresh()
    }

    private func applyVolume() {
        engine.setVolume(muted ? 0 : Float(volume))
    }

    func recentlyStuttered(_ s: StreamInfo) -> Bool {
        guard let seen = stutterSeen[s.id] else { return false }
        return Date().timeIntervalSince(seen.at) < 10
    }

    /// Received sound level before the volume, 0...1.
    var receiveLevel: Double {
        Double(state.streams.map(\.level).max() ?? 0).clamped(0, 1)
    }

    // MARK: devices

    /// Devices found on the network plus ones added by IP.
    var devices: [DeviceItem] {
        var out: [DeviceItem] = []
        var seen = Set<String>()
        let mine = Set(state.ips)
        for p in state.peers where !mine.contains(p.ip) && !seen.contains(p.ip) {
            seen.insert(p.ip)
            out.append(DeviceItem(ip: p.ip, name: p.name, dest: p.addr, receiving: p.receiving, manual: false))
        }
        for ip in manualIPs where !seen.contains(ip) {
            seen.insert(ip)
            out.append(DeviceItem(ip: ip, name: ip, dest: ip, receiving: true, manual: true))
        }
        return out
    }

    private func dests(_ picked: Set<String>) -> [String] {
        var out = devices.filter { picked.contains($0.ip) }.map(\.dest)
        // Ticked devices that are offline right now still get audio when they come back.
        for ip in picked where !out.contains(where: { $0 == ip || $0.hasPrefix(ip + ":") }) {
            out.append(ip)
        }
        return out
    }

    private func applyTargets() {
        engine.setSendDests(dests(sendTargets))
        pushConfig(force: true)
    }

    private func applyForward() {
        engine.setForward(dests(forwardTargets))
    }

    func toggleSend(_ ip: String) {
        if sendTargets.contains(ip) { sendTargets.remove(ip) } else { sendTargets.insert(ip) }
    }

    func toggleForward(_ ip: String) {
        if forwardTargets.contains(ip) { forwardTargets.remove(ip) } else { forwardTargets.insert(ip) }
    }

    func addManual(_ raw: String) -> Bool {
        let ip = raw.trimmingCharacters(in: .whitespaces)
        let parts = ip.split(separator: ".")
        guard parts.count == 4, parts.allSatisfy({ UInt8($0) != nil }) else {
            show("IP ไม่ถูกต้อง")
            return false
        }
        if !manualIPs.contains(ip) { manualIPs.append(ip) }
        return true
    }

    func removeManual(_ ip: String) {
        manualIPs.removeAll { $0 == ip }
        sendTargets.remove(ip)
        forwardTargets.remove(ip)
    }

    func rename(_ name: String) {
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !n.isEmpty else { return }
        d.set(n, forKey: "name")
        engine.setName(n)
        pushConfig(force: true)
        refresh()
    }

    // MARK: sending

    /// Screen broadcast is live (the extension reported in the last 1.5 s).
    var broadcasting: Bool { report?.sending ?? false }
    var sending: Bool { micSending || broadcasting }

    var captureMs: Double { broadcasting ? report?.captureMs ?? 0 : state.captureMs }

    func startMic() {
        guard !sendTargets.isEmpty else {
            show("เลือกเครื่องปลายทางอย่างน้อย 1 เครื่องก่อน")
            return
        }
        AVAudioSession.sharedInstance().requestRecordPermission { ok in
            Task { @MainActor [weak self] in
                guard let self = self else { return }
                guard ok else {
                    self.show("ต้องอนุญาตไมโครโฟนก่อน (ตั้งค่า > Stream Sound > ไมโครโฟน)")
                    return
                }
                if let err = self.engine.startSending(self.dests(self.sendTargets)) {
                    self.show("ส่งเสียงไม่ได้: \(err)")
                    return
                }
                self.micSending = true
                if let err = self.audio.set(play: self.state.receiving, mic: true) {
                    self.show(err)
                    self.stopMic()
                }
                self.refresh()
            }
        }
    }

    func stopMic() {
        engine.stopSending()
        micSending = false
        if let err = audio.set(play: state.receiving, mic: false) { show(err) }
        refresh()
    }

    /// Keep the broadcast extension up to date with where to send.
    func pushConfig(force: Bool) {
        let cfg = BroadcastConfig(dests: dests(sendTargets), mode: mode, name: state.name.isEmpty ? UIDevice.current.name : state.name)
        let text = cfg.encode()
        if force || text != lastConfigSent || Date().timeIntervalSince(lastConfigAt) > 1 {
            LoopLink.send(text, to: LoopLink.extensionPort)
            lastConfigSent = text
            lastConfigAt = Date()
        }
    }

    func stopBroadcast() {
        var cfg = BroadcastConfig(dests: [], mode: mode, name: state.name)
        cfg.stop = true
        LoopLink.send(cfg.encode(), to: LoopLink.extensionPort)
    }

    // MARK: visualizer

    /// Fill `buf` with the current picture for one side; false when silent/off.
    func scope(send: Bool, kind: VizStyle, into buf: inout [Float]) -> Bool {
        if send && broadcasting, let r = report {
            let src = kind == .bars ? r.spectrum : r.waveform
            guard !src.isEmpty else { return false }
            for i in buf.indices {
                buf[i] = src[min(src.count - 1, i * src.count / max(buf.count, 1))]
            }
            return true
        }
        return engine.scope(tap: send ? 0 : 1, kind: kind == .bars ? 0 : 1, into: &buf)
    }

    // MARK: misc

    private func applyIdle() {
        let busy = sending || !state.streams.isEmpty
        let want = keepAwake && busy
        if UIApplication.shared.isIdleTimerDisabled != want {
            UIApplication.shared.isIdleTimerDisabled = want
        }
    }

    func checkUpdate(quiet: Bool) async {
        checkingUpdate = true
        lastUpdateCheck = Date()
        if !quiet { updateText = "กำลังตรวจสอบอัปเดต…" }
        defer { checkingUpdate = false }
        do {
            switch try await Updater.check(beta: allowBeta) {
            case .upToDate:
                newer = nil
                updateText = "เป็นเวอร์ชันล่าสุดแล้ว"
            case .noRelease:
                newer = nil
                updateText = "ยังไม่มีเวอร์ชันที่เผยแพร่บน GitHub"
            case .newer(let r):
                newer = r
                updateText = "มีเวอร์ชัน \(r.version)\(r.beta ? " (เบต้า)" : "") ให้อัปเดต"
            }
        } catch {
            if !quiet { updateText = "ตรวจสอบอัปเดตไม่ได้: \(error.localizedDescription)" }
        }
    }

    func installUpdate(addSource: Bool) {
        let beta = addSource ? nil : newer.flatMap { $0.beta ? $0 : nil }
        Updater.openInstaller(addSource: addSource, beta: beta) { [weak self] msg in self?.show(msg) }
    }

    func quit() {
        if micSending { stopMic() }
        if broadcasting { stopBroadcast() }
        engine.stopReceiving()
        audio.set(play: false, mic: false)
        UIApplication.shared.isIdleTimerDisabled = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { exit(0) }
    }
}

extension Double {
    func clamped(_ lo: Double, _ hi: Double) -> Double { Swift.min(hi, Swift.max(lo, self)) }
}
