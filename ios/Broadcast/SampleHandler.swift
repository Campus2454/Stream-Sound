import AVFoundation
import CoreMedia
import Foundation
import ReplayKit

/// Screen-broadcast extension: iOS hands it everything the iPhone plays
/// ("app audio", all apps mixed; DRM-protected audio arrives silent) and it
/// streams that to the devices the app picked. Video frames are ignored.
///
/// It runs as its own process. The app sends it the destinations over
/// loopback (LoopLink); the last ones are remembered so a broadcast started
/// from Control Center still goes somewhere.
final class SampleHandler: RPBroadcastSampleHandler {
    private var engine: Ssnd?
    private var link: LoopLink?
    private var timer: DispatchSourceTimer?
    private let queue = DispatchQueue(label: "ssnd.broadcast")
    private var config = BroadcastConfig(dests: [], mode: "balanced", name: "iPhone")
    private var converter = PCMConverter()
    private var note = ""
    private var delayMs = 0.0
    private let defaults = UserDefaults.standard

    override func broadcastStarted(withSetupInfo setupInfo: [String: NSObject]?) {
        if let d = defaults.string(forKey: "config").flatMap({ BroadcastConfig.decode(Data($0.utf8)) }) {
            config = d
        }
        config.stop = false
        guard let e = Ssnd(name: config.name, mode: config.mode, announce: false) else {
            finish("เริ่มระบบเสียงไม่ได้")
            return
        }
        engine = e
        link = LoopLink(listen: LoopLink.extensionPort)
        if let err = e.startSending(config.dests) {
            note = err
        }
        let t = DispatchSource.makeTimerSource(queue: queue)
        t.schedule(deadline: .now(), repeating: .milliseconds(50))
        t.setEventHandler { [weak self] in self?.tick() }
        t.resume()
        timer = t
    }

    /// 20 times a second: pick up new settings, report back to the app.
    private func tick() {
        guard let e = engine else { return }
        if let data = link?.receive(), let c = BroadcastConfig.decode(data) {
            if c.stop {
                finish("หยุดส่งจากแอปแล้ว")
                return
            }
            if c.dests != config.dests { e.setSendDests(c.dests) }
            if c.mode != config.mode { e.setMode(c.mode) }
            if c.name != config.name { e.setName(c.name) }
            config = c
            defaults.set(c.encode(), forKey: "config")
        }
        if config.dests.isEmpty {
            note = "ยังไม่ได้เลือกเครื่องปลายทาง เปิดแอปแล้วเลือกเครื่อง"
        } else if note.hasPrefix("ยังไม่ได้เลือก") {
            note = ""
        }
        _ = ssnd_link_report(e.h, LoopLink.appPort, note)
    }

    override func processSampleBuffer(_ sampleBuffer: CMSampleBuffer, with sampleBufferType: RPSampleBufferType) {
        guard sampleBufferType == .audioApp, let e = engine else { return }
        guard let out = converter.convert(sampleBuffer) else { return }
        out.samples.withUnsafeBufferPointer { p in
            ssnd_push_capture(e.h, p.baseAddress, UInt32(out.count), out.rate, 2)
        }
        // How long ago this audio was played: ReplayKit's own delay.
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        let dur = CMSampleBufferGetDuration(sampleBuffer)
        let now = CMClockGetTime(CMClockGetHostTimeClock())
        if pts.isValid {
            let end = dur.isValid ? CMTimeAdd(pts, dur) : pts
            let ms = CMTimeGetSeconds(CMTimeSubtract(now, end)) * 1000
            if ms.isFinite, ms >= 0, ms < 1000 {
                delayMs = delayMs == 0 ? ms : delayMs * 0.9 + ms * 0.1
                Ssnd.setCaptureLatency(ms: delayMs + Double(out.count / 2) / Double(out.rate) * 1000)
            }
        }
    }

    override func broadcastFinished() {
        timer?.cancel()
        timer = nil
        engine?.stopSending()
        if let e = engine {
            // One last report so the app flips to "stopped" at once.
            queue.sync { _ = ssnd_link_report(e.h, LoopLink.appPort, "stopped") }
        }
        engine = nil
        link = nil
    }

    private func finish(_ why: String) {
        let err = NSError(domain: "StreamSound", code: 1, userInfo: [NSLocalizedDescriptionKey: why])
        finishBroadcastWithError(err)
    }
}

/// Turns ReplayKit's audio (16-bit, sometimes big-endian, mono or stereo,
/// interleaved or not, float on some iOS versions) into interleaved stereo float.
struct PCMConverter {
    struct Output {
        var samples: [Float]
        var count: Int
        var rate: UInt32
    }

    private var out = Output(samples: [Float](repeating: 0, count: 8192), count: 0, rate: 48000)

    mutating func convert(_ sb: CMSampleBuffer) -> Output? {
        guard let fmt = CMSampleBufferGetFormatDescription(sb),
              let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(fmt) else { return nil }
        let asbd = asbdPtr.pointee
        guard asbd.mFormatID == kAudioFormatLinearPCM, asbd.mSampleRate >= 8000 else { return nil }
        let frames = CMSampleBufferGetNumSamples(sb)
        guard frames > 0 else { return nil }

        var ablSize = 0
        CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sb, bufferListSizeNeededOut: &ablSize, bufferListOut: nil, bufferListSize: 0,
            blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: nil)
        guard ablSize > 0 else { return nil }
        let ablMem = UnsafeMutableRawPointer.allocate(byteCount: ablSize, alignment: 16)
        defer { ablMem.deallocate() }
        let abl = ablMem.bindMemory(to: AudioBufferList.self, capacity: 1)
        var block: CMBlockBuffer?
        let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sb, bufferListSizeNeededOut: nil, bufferListOut: abl, bufferListSize: ablSize,
            blockBufferAllocator: nil, blockBufferMemoryAllocator: nil,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, blockBufferOut: &block)
        guard status == noErr else { return nil }

        let flags = asbd.mFormatFlags
        let isFloat = flags & kAudioFormatFlagIsFloat != 0
        let bigEndian = flags & kAudioFormatFlagIsBigEndian != 0
        let nonInterleaved = flags & kAudioFormatFlagIsNonInterleaved != 0
        let bits = Int(asbd.mBitsPerChannel)
        let ch = max(1, Int(asbd.mChannelsPerFrame))
        let buffers = UnsafeMutableAudioBufferListPointer(abl)

        let need = frames * 2
        if out.samples.count < need {
            out.samples = [Float](repeating: 0, count: need)
        }

        // Sample `c` of frame `f`, as float.
        func read(_ f: Int, _ c: Int) -> Float {
            let bufIndex = nonInterleaved ? min(c, buffers.count - 1) : 0
            guard bufIndex >= 0, bufIndex < buffers.count, let base = buffers[bufIndex].mData else { return 0 }
            let stride = nonInterleaved ? 1 : ch
            let idx = f * stride + (nonInterleaved ? 0 : min(c, ch - 1))
            let byteLen = Int(buffers[bufIndex].mDataByteSize)
            switch (isFloat, bits) {
            case (true, 32):
                guard (idx + 1) * 4 <= byteLen else { return 0 }
                var u = base.loadUnaligned(fromByteOffset: idx * 4, as: UInt32.self)
                if bigEndian { u = u.byteSwapped }
                return Float(bitPattern: u)
            case (false, 16):
                guard (idx + 1) * 2 <= byteLen else { return 0 }
                var u = base.loadUnaligned(fromByteOffset: idx * 2, as: UInt16.self)
                if bigEndian { u = u.byteSwapped }
                return Float(Int16(bitPattern: u)) / 32768
            case (false, 32):
                guard (idx + 1) * 4 <= byteLen else { return 0 }
                var u = base.loadUnaligned(fromByteOffset: idx * 4, as: UInt32.self)
                if bigEndian { u = u.byteSwapped }
                return Float(Int32(bitPattern: u)) / 2_147_483_648
            default:
                return 0
            }
        }

        for f in 0..<frames {
            let l = read(f, 0)
            let r = ch > 1 ? read(f, 1) : l
            out.samples[2 * f] = l
            out.samples[2 * f + 1] = r
        }
        out.count = need
        out.rate = UInt32(asbd.mSampleRate.rounded())
        return out
    }
}
