import AVFoundation

/// The iPhone's speaker and microphone. Received audio is pulled straight
/// from the Rust mixer on the render thread (AVAudioSourceNode); microphone
/// audio is pushed straight into the engine (AVAudioSinkNode). Both run at
/// the smallest IO buffer iOS allows (~5 ms).
///
/// Rebuilt from scratch whenever something changes underneath (headphones
/// plugged in, a phone call ended, media services reset).
final class AudioIO {
    private let engine: Ssnd
    private var av: AVAudioEngine?
    private var wantPlay = false
    private var wantMic = false
    private var observers: [NSObjectProtocol] = []
    private var configObserver: NSObjectProtocol?
    /// Called on the main thread when audio stops by itself.
    var onProblem: ((String) -> Void)?

    init(engine: Ssnd) {
        self.engine = engine
        let nc = NotificationCenter.default
        observers.append(nc.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] n in
            self?.interrupted(n)
        })
        observers.append(nc.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            self?.restart()
        })
        observers.append(nc.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] _ in
            self?.noteLatency()
        })
    }

    deinit {
        for o in observers { NotificationCenter.default.removeObserver(o) }
        teardown()
    }

    var isRunning: Bool { av?.isRunning ?? false }

    /// Play received audio and/or capture the microphone. nil on success.
    @discardableResult
    func set(play: Bool, mic: Bool) -> String? {
        if play == wantPlay && mic == wantMic && (isRunning || !(play || mic)) {
            return nil
        }
        wantPlay = play
        wantMic = mic
        return restart()
    }

    @discardableResult
    func restart() -> String? {
        teardown()
        let s = AVAudioSession.sharedInstance()
        guard wantPlay || wantMic else {
            try? s.setActive(false, options: .notifyOthersOnDeactivation)
            return nil
        }
        do {
            if wantMic {
                try s.setCategory(.playAndRecord, mode: .default,
                                  options: [.mixWithOthers, .defaultToSpeaker, .allowBluetoothA2DP])
            } else {
                try s.setCategory(.playback, mode: .default, options: [.mixWithOthers])
            }
            try? s.setPreferredSampleRate(48_000)
            try? s.setPreferredIOBufferDuration(0.005)
            try s.setActive(true)
        } catch {
            return "เปิดระบบเสียงไม่ได้: \(error.localizedDescription)"
        }

        let av = AVAudioEngine()
        let h = engine.h
        if wantPlay {
            let rate = s.sampleRate > 0 ? s.sampleRate : 48_000
            guard let fmt = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 2) else {
                return "รูปแบบเสียงไม่รองรับ"
            }
            let outRate = UInt32(rate)
            let src = AVAudioSourceNode(format: fmt) { _, _, frames, abl -> OSStatus in
                let bufs = UnsafeMutableAudioBufferListPointer(abl)
                let l = bufs.count > 0 ? bufs[0].mData?.assumingMemoryBound(to: Float.self) : nil
                let r = bufs.count > 1 ? bufs[1].mData?.assumingMemoryBound(to: Float.self) : nil
                ssnd_render_planar(h, l, r, frames, outRate)
                return noErr
            }
            av.attach(src)
            av.connect(src, to: av.mainMixerNode, format: fmt)
        }
        if wantMic {
            let input = av.inputNode
            let inFmt = input.outputFormat(forBus: 0)
            guard inFmt.sampleRate > 0, inFmt.channelCount > 0 else {
                return "ไม่พบไมโครโฟน"
            }
            let inRate = UInt32(inFmt.sampleRate)
            let sink = AVAudioSinkNode { _, frames, abl -> OSStatus in
                let bufs = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: abl))
                guard bufs.count > 0, let l = bufs[0].mData?.assumingMemoryBound(to: Float.self) else { return noErr }
                let r = bufs.count > 1 ? bufs[1].mData?.assumingMemoryBound(to: Float.self) : nil
                ssnd_push_capture_planar(h, UnsafePointer(l), r.map { UnsafePointer($0) }, frames, inRate)
                return noErr
            }
            av.attach(sink)
            av.connect(input, to: sink, format: inFmt)
        }
        av.prepare()
        do {
            try av.start()
        } catch {
            return "เริ่มเสียงไม่ได้: \(error.localizedDescription)"
        }
        self.av = av
        configObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange, object: av, queue: .main
        ) { [weak self] _ in
            if let err = self?.restart() { self?.onProblem?(err) }
        }
        noteLatency()
        return nil
    }

    private func teardown() {
        if let o = configObserver {
            NotificationCenter.default.removeObserver(o)
            configObserver = nil
        }
        av?.stop()
        av = nil
    }

    private func interrupted(_ n: Notification) {
        guard let raw = n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
        if type == .ended, wantPlay || wantMic {
            if let err = restart() { onProblem?(err) }
        }
    }

    /// Tell the engine how long sound spends in iOS's own buffers.
    private func noteLatency() {
        let s = AVAudioSession.sharedInstance()
        Ssnd.setOutputLatency(ms: (s.outputLatency + s.ioBufferDuration) * 1000)
        if wantMic {
            Ssnd.setCaptureLatency(ms: (s.inputLatency + s.ioBufferDuration) * 1000)
        }
    }
}
