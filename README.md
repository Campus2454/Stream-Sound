# Stream Sound (prototype v0.1)

Low-latency audio streaming between devices on the same LAN. One Rust engine
shared by every platform; desktop UI in Thai.

## Builds

Every push builds Windows, Linux and Android automatically: open the
**Actions** tab, pick the latest green run, and download
`StreamSound-windows-x64`, `StreamSound-linux-x64` or `StreamSound-android`
under *Artifacts* (they arrive as .zip files).

| Platform | Status |
|---|---|
| Windows 10/11 | Built and unit-tested on GitHub's Windows runner; not yet tried on a real PC |
| Linux (PulseAudio or PipeWire) | Built and tested end to end |
| Android 10+ | Built on GitHub; not yet tried on a real phone |
| iOS | Planned: cloud Mac build, sideloaded from Windows (receiver first) |

## How to use (ภาษาไทย)

1. เปิดแอปบนทุกเครื่องที่ต่อ Wi-Fi/LAN วงเดียวกัน เครื่องต่าง ๆ จะขึ้นในรายการเอง
   (ถ้าไม่ขึ้น ให้พิมพ์ IP ในช่อง "อุปกรณ์" แล้วกด "เพิ่ม")
2. เครื่องที่จะฟัง: ติ๊ก "เปิดรับเสียงจากเครื่องอื่น" (เปิดไว้อยู่แล้วตอนเริ่มแอป)
3. เครื่องที่จะส่ง: เลือก "แหล่งเสียง" (เสียงทั้งระบบ / แอปใดแอปหนึ่ง / ไมค์ / เสียงทดสอบ)
   ติ๊กเครื่องปลายทางได้หลายเครื่อง แล้วกด "เริ่มส่งเสียง"
4. ส่งต่อเป็นทอด: บนเครื่องกลาง เปิด "ส่งต่อเสียงที่รับได้ไปเครื่องอื่น" แล้วติ๊กเครื่องถัดไป
   (ปิด "เล่นเสียงที่เครื่องนี้" ได้ถ้าไม่อยากให้เครื่องกลางดังด้วย)
5. หลายเครื่องส่งมาเครื่องเดียว: ให้ทุกเครื่องส่งมาที่เครื่องเดียวกัน เสียงจะถูกมิกซ์รวมกันเอง
6. "บัฟเฟอร์": น้อย = หน่วงต่ำ, มาก = ทนสัญญาณ Wi-Fi สะดุดได้ดีกว่า (สาย LAN ลองที่ 5–10 ms, Wi-Fi 20–40 ms)

**Windows:** the first time it runs, Windows Firewall asks about network access.
Allow **Private networks**, otherwise nothing can reach this PC. SmartScreen may
warn because the exe is unsigned: "More info" → "Run anyway".

**Android:** unzip `StreamSound-android`, copy `StreamSound.apk` to the phone and
open it (allow "install unknown apps" for your file manager or browser).
Sending system or app audio asks for the screen-recording permission; only
audio is used. Apps that opt out of capture (some streaming apps) stay silent.
Every build is signed with the same key, so new versions install over old ones.

## Command line (testing / headless relay)

```
stream-sound recv  [--port 47800] [--latency 20] [--forward IP[:PORT],..] [--no-play]
stream-sound send  --to IP[:PORT],.. [--source system|tone|app:NAME|input:NAME]
stream-sound sources     # list capturable sources
stream-sound peers       # list devices on the LAN
```

## How it works

- **Transport:** UDP, port 47800 (audio) and 47801 (discovery broadcast).
  5 ms packets of uncompressed 16-bit PCM at the source's sample rate
  (≈1.5 Mbit/s for 48 kHz stereo: lossless, no codec delay).
- **Receiver:** one jitter buffer per source, adaptive target (grows 5 ms after an
  underrun, shrinks back after 20 s stable), gentle playback-speed correction
  (±0.6 %) for clock drift, Catmull-Rom resampling to the speaker's rate,
  packet-loss concealment, then all sources mixed with a soft limiter.
- **Daisy-chain:** a relay forwards each packet the moment it arrives, before any
  buffering, so each hop adds well under 1 ms. A hop counter stops loops.
- **Stability:** capture and playback run in their own threads and reconnect by
  themselves when a device changes or disappears; panics inside the network
  loop are caught and the loop restarts.
- **Android:** the Kotlin app (`android/`) owns the speaker (low-latency
  `AudioTrack`) and capture (`AudioRecord` with playback capture or the mic)
  and moves audio in and out of the same Rust engine through JNI
  (`android/rust`). A foreground service with Wi-Fi low-latency and wake locks
  keeps it running with the screen off.
- **Capture:**
  - Windows: WASAPI loopback (whole system), process loopback (one app, needs
    Windows 10 2004+), any input device.
  - Linux: PulseAudio/PipeWire monitor (whole system); per-app by moving the
    app's streams to a private sink, which is removed on stop.

## Tested here (Linux, 2026-09-29)

- Tone and per-app (paplay) capture streamed and recorded back from the speaker
  output: correct pitch, zero clicks or dropouts.
- One sender to two receivers, two senders mixed on one receiver, and a
  two-hop daisy-chain: 125 s soak, 25,000 packets, 0 lost, 0 underruns.
- Measured path latency is roughly 5 ms capture + 5 ms packet + the buffer
  setting (20 ms default) + ~15 ms speaker buffer.

## Build from source

```
cargo build --release                                   # Linux (needs libpulse-dev, libasound2-dev)
cargo build --release --target x86_64-pc-windows-gnu    # Windows (needs mingw-w64)
cargo test -p ssnd-core
```

Font: Loma from TLWG (GPL with font exception).
