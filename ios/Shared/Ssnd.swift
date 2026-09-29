import Foundation

/// Swift face of the Rust engine (ios/rust). One instance per process.
final class Ssnd {
    let h: OpaquePointer

    init?(name: String, mode: String, announce: Bool) {
        guard let h = ssnd_create(name, mode, announce) else { return nil }
        self.h = h
    }

    deinit { ssnd_destroy(h) }

    /// Takes ownership of a string the engine returned.
    static func take(_ p: UnsafeMutablePointer<CChar>?) -> String? {
        guard let p = p else { return nil }
        defer { ssnd_free_string(p) }
        return String(cString: p)
    }

    /// nil on success, otherwise the error.
    func startReceiving() -> String? { Ssnd.take(ssnd_start_receiving(h)) }
    func stopReceiving() { ssnd_stop_receiving(h) }

    func startSending(_ dests: [String]) -> String? {
        Ssnd.take(ssnd_start_sending(h, dests.joined(separator: ",")))
    }
    func stopSending() { ssnd_stop_sending(h) }
    func setSendDests(_ dests: [String]) { ssnd_set_send_dests(h, dests.joined(separator: ",")) }

    func setForward(_ dests: [String]) { ssnd_set_forward(h, dests.joined(separator: ",")) }
    func setPlayLocal(_ on: Bool) { ssnd_set_play_local(h, on) }
    func setVolume(_ v: Float) { ssnd_set_volume(h, v) }
    func setMode(_ m: String) { ssnd_set_mode(h, m) }
    func setName(_ n: String) { ssnd_set_name(h, n) }
    func setManualPeers(_ ips: [String]) { ssnd_set_manual_peers(h, ips.joined(separator: ",")) }

    static func setOutputLatency(ms: Double) { ssnd_set_output_latency(Float(ms)) }
    static func setCaptureLatency(ms: Double) { ssnd_set_capture_latency(Float(ms)) }

    /// tap 0 = send, 1 = receive; kind 0 = spectrum, 1 = waveform.
    func scope(tap: Int32, kind: Int32, into buf: inout [Float]) -> Bool {
        let n = UInt32(buf.count)
        return buf.withUnsafeMutableBufferPointer { ssnd_scope(h, tap, kind, n, $0.baseAddress) }
    }

    func stateJSON() -> Data {
        Data((Ssnd.take(ssnd_state_json(h)) ?? "{}").utf8)
    }
}

/// App <-> broadcast extension messages over loopback UDP (ios/rust/src/link.rs).
final class LoopLink {
    static let extensionPort: UInt16 = 47802
    static let appPort: UInt16 = 47803

    private let p: OpaquePointer
    private var buf = [UInt8](repeating: 0, count: 4096)

    init?(listen port: UInt16) {
        guard let p = ssnd_link_open(port) else { return nil }
        self.p = p
    }

    deinit { ssnd_link_close(p) }

    /// Newest message since the last call, if any.
    func receive() -> Data? {
        let cap = UInt32(buf.count)
        let n = buf.withUnsafeMutableBufferPointer { ssnd_link_recv(p, $0.baseAddress, cap) }
        return n > 0 ? Data(buf[0..<Int(n)]) : nil
    }

    @discardableResult
    static func send(_ text: String, to port: UInt16) -> Bool {
        var bytes = Array(text.utf8)
        let len = UInt32(bytes.count)
        return bytes.withUnsafeMutableBufferPointer { ssnd_link_send(port, $0.baseAddress, len) }
    }
}

/// What the app tells the broadcast extension, one line per field.
struct BroadcastConfig: Equatable {
    var dests: [String]
    var mode: String
    var name: String
    var stop = false

    func encode() -> String {
        ["SSNDBC1", dests.joined(separator: ","), mode, name.replacingOccurrences(of: "\n", with: " "), stop ? "stop" : ""]
            .joined(separator: "\n")
    }

    static func decode(_ data: Data) -> BroadcastConfig? {
        guard let s = String(data: data, encoding: .utf8) else { return nil }
        let f = s.components(separatedBy: "\n")
        guard f.count >= 4, f[0] == "SSNDBC1" else { return nil }
        let dests = f[1].split(separator: ",").map(String.init).filter { !$0.isEmpty }
        return BroadcastConfig(dests: dests, mode: f[2], name: f[3], stop: f.count > 4 && f[4] == "stop")
    }
}

/// What the extension reports back (built in Rust by ssnd_link_report).
struct BroadcastReport: Decodable {
    var sending: Bool
    var packets: UInt64
    var level: Float
    var captureMs: Double
    var error: String
    var note: String
    var dests: [String]
    var spectrum: [Float]
    var waveform: [Float]
}
