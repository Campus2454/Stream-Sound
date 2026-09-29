# Stream Sound (prototype v0.1)

Low-latency audio streaming between devices on the same LAN. One Rust engine
shared by every platform; desktop UI in Thai.

## Builds

Every push builds Windows, Linux and Android automatically, and every change
on `main` is published as a release that installed apps update from. See
**การติดตั้ง** below for where to download them and how to install.

| Platform | Status |
|---|---|
| Windows 10/11 | Built and unit-tested on GitHub's Windows runner; not yet tried on a real PC |
| Linux (PulseAudio or PipeWire) | Built and tested end to end; one standalone file |
| Android 10+ | Built on GitHub; not yet tried on a real phone |
| iOS | Not built yet (planned: cloud Mac build, sideloaded from Windows) |

## การติดตั้ง

### ดาวน์โหลดไฟล์

เปิด <https://github.com/Campus2454/Audio-Streaming/releases/latest>
(ไม่ต้องล็อกอิน) แล้วเลื่อนลงไปที่ **Assets**:

| เครื่อง | ไฟล์ |
|---|---|
| Windows | `StreamSound-windows-x64.zip` |
| Linux | `StreamSound-linux-x86_64` |
| Android | `StreamSound.apk` |

ติดตั้งครั้งแรกครั้งเดียว หลังจากนั้นแอปจะอัปเดตตัวเองจากหน้านี้ (ดู **อัปเดตอัตโนมัติ** ด้านล่าง)

ไฟล์ทดสอบที่ยังไม่รวมเข้า `main` อยู่ที่ <https://github.com/Campus2454/Audio-Streaming/actions>
(ต้องล็อกอิน): กดรายการที่มี **✓ สีเขียว** แล้วดาวน์โหลดจากหัวข้อ **Artifacts** (ได้เป็นไฟล์ .zip)

### Windows 10/11

1. คลิกขวาไฟล์ zip > **Extract All** ไปไว้ในโฟลเดอร์ของคุณเอง เช่น `Documents\StreamSound`
   (ต้องแตกไฟล์ก่อน อย่าเปิดจากใน zip และอย่าไว้ใน `Program Files` ไม่อย่างนั้นแอปจะอัปเดตตัวเองไม่ได้)
2. เปิด `StreamSound.exe`
3. ถ้าขึ้น "Windows protected your PC" ให้กด **More info** > **Run anyway**
4. ถ้า Windows Firewall ถาม ให้ติ๊ก **Private networks** แล้วกด **Allow**
   (ถ้าไม่อนุญาต เครื่องอื่นจะส่งเสียงมาเครื่องนี้ไม่ได้)

### Android 10 ขึ้นไป

1. เปิดลิงก์ release ด้านบนบนมือถือ แล้วแตะ `StreamSound.apk`
   (หรือโหลดบนคอม แล้วส่งไฟล์เข้ามือถือ)
2. แตะไฟล์ที่ดาวน์โหลดมา > ถ้ามือถือถาม ให้เปิด **อนุญาตจากแหล่งที่มานี้**
   (Install unknown apps) ให้เบราว์เซอร์หรือแอปจัดการไฟล์ แล้วกด **ติดตั้ง**
   - ถ้า Play Protect เตือน ให้กด **รายละเอียดเพิ่มเติม** > **ติดตั้งต่อไป**
3. เปิดแอป แล้วกด **อนุญาต** การแจ้งเตือน (แอปใช้แจ้งเตือนเพื่อทำงานตอนปิดจอ)
4. ตอนกด "เริ่มส่งเสียง" ครั้งแรก:
   - อนุญาต **บันทึกเสียง**
   - เมื่อ Android ถามเรื่องบันทึก/แชร์หน้าจอ ให้เลือก **ทั้งหน้าจอ** แล้วกด **เริ่ม**
     (แอปใช้แค่เสียง ไม่ได้บันทึกภาพ)
5. แนะนำ: ตั้งค่า > แอป > Stream Sound > แบตเตอรี่ > **ไม่จำกัด (Unrestricted)**
   เพื่อไม่ให้มือถือปิดแอปเองตอนสตรีมนาน ๆ

แอปบางตัว (เช่นแอปดูหนังบางแอป) ไม่ยอมให้จับเสียง จะส่งออกไปเป็นเสียงเงียบ

### Linux (Arch Linux)

`StreamSound-linux-x86_64` เป็นไฟล์เดียวจบ ไม่ต้องติดตั้งแพ็กเกจเพิ่ม ใช้แค่ glibc
และเซิร์ฟเวอร์เสียงที่เดสก์ท็อปมีอยู่แล้ว (PulseAudio หรือ PipeWire)
ใช้ได้กับ Arch และดิสโทรอื่นที่ใหม่กว่า Ubuntu 22.04

1. ติดตั้งไว้ในโฟลเดอร์ของคุณเอง แล้วเปิด:
   ```
   install -Dm755 ~/Downloads/StreamSound-linux-x86_64 ~/.local/bin/stream-sound
   ~/.local/bin/stream-sound
   ```
   (ไว้ที่ไหนก็ได้ที่คุณเขียนไฟล์ได้ เพื่อให้แอปอัปเดตตัวเองได้ อย่าไว้ใน `/usr/bin`)
2. ถ้าขึ้นว่า "ไม่พบ PulseAudio/PipeWire (libpulse)": เครื่องที่ใช้ PipeWire ต้องมี
   `pipewire-pulse` ด้วย: `sudo pacman -S --needed libpulse pipewire-pulse`
   (ตรวจได้ด้วย `pactl info` ถ้าขึ้น `Server Name` แสดงว่าใช้ได้)
3. การส่งเสียงเฉพาะแอปใช้คำสั่ง `pactl` ซึ่งมากับแพ็กเกจ `libpulse` บน Arch อยู่แล้ว
4. ถ้าเปิดไฟร์วอลล์ไว้ ให้เปิดพอร์ต UDP 47800–47801
   (ufw: `sudo ufw allow 47800:47801/udp`,
   firewalld: `sudo firewall-cmd --permanent --add-port=47800-47801/udp && sudo firewall-cmd --reload`)

### อัปเดตอัตโนมัติ

ทุกครั้งที่มีการแก้โค้ดเข้า `main` GitHub จะสร้าง release ใหม่ (build ใหม่) ให้เอง

- **Windows / Linux:** แอปเช็กตอนเปิดและทุก 6 ชั่วโมง ถ้ามีเวอร์ชันใหม่จะดาวน์โหลดเก็บไว้เงียบ ๆ
  แล้วขึ้นแถบสีแดง "มีเวอร์ชันใหม่พร้อมแล้ว" กด **รีสตาร์ตเพื่ออัปเดต** เมื่อสะดวก
  (แอปไม่รีสตาร์ตเองกลางการสตรีม) ด้านล่างสุดมีเลข build ปัจจุบันและปุ่ม **ตรวจสอบอัปเดต**
- **Android:** แอปเช็กและดาวน์โหลดตอนเปิด แล้วขึ้นการ์ด **ติดตั้งอัปเดต**
  ครั้งแรก Android จะพาไปหน้า "ติดตั้งแอปที่ไม่รู้จัก" ให้เปิดอนุญาตให้ Stream Sound
  แล้วกลับมากดอีกครั้ง Android จะถามยืนยันการติดตั้งทุกครั้ง (ข้ามไม่ได้)
- **iPhone:** แอปที่ sideload อัปเดตตัวเองไม่ได้ (ข้อจำกัดของ Apple)
  เมื่อมีแอป iOS จะทำ source สำหรับ SideStore ให้ดึงเวอร์ชันใหม่จาก GitHub
- **Command line:** `stream-sound update`

### iPhone / iPad (ยังไม่พร้อม)

ตอนนี้ **ยังไม่มีไฟล์แอปสำหรับ iOS** ขั้นตอนด้านล่างคือวิธีที่จะใช้เมื่อแอปพร้อม
โดยไม่ต้องมี Mac:

- ไฟล์ `.ipa` จะถูกสร้างบนเครื่อง Mac ในคลาวด์ของ GitHub แล้วดาวน์โหลดจาก Artifacts เหมือนแพลตฟอร์มอื่น
- ติดตั้งจากคอม Windows ด้วย **Sideloadly** และ Apple ID ของคุณเอง (ใช้ Apple ID ฟรีได้)
  1. ติดตั้ง iTunes และ iCloud เวอร์ชันจากเว็บ Apple (ไม่ใช่จาก Microsoft Store) และ Sideloadly
  2. ต่อ iPhone กับคอมด้วยสาย กด **Trust** บน iPhone
  3. ลากไฟล์ `.ipa` ใส่ Sideloadly ใส่ Apple ID แล้วกด **Start**
  4. บน iPhone: ตั้งค่า > ทั่วไป > VPN และการจัดการอุปกรณ์ > กด **เชื่อถือ** Apple ID ของคุณ
  5. iOS 16 ขึ้นไป: ตั้งค่า > ความเป็นส่วนตัวและความปลอดภัย > **โหมดนักพัฒนา** > เปิด แล้วรีสตาร์ต
- ข้อจำกัดของ Apple ID ฟรี: แอปจะใช้ได้ **7 วัน** แล้วต้องติดตั้งซ้ำ (หรือเปิด auto-refresh
  ใน Sideloadly / ใช้ SideStore ให้ต่ออายุเองบนมือถือ) และติดตั้งแอปแบบนี้ได้พร้อมกันไม่เกิน 3 แอป
  ถ้าสมัคร Apple Developer (99 USD/ปี) จะใช้ได้ 1 ปี
- iPhone จะเริ่มจาก **รับเสียง** ก่อน การส่งเสียงจาก iPhone ทำได้ผ่าน "บันทึกหน้าจอ" เท่านั้น
  เลือกเฉพาะแอปไม่ได้ และแอปที่มีลิขสิทธิ์ (เช่น Apple Music) จะเป็นเสียงเงียบ

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

## Command line (testing / headless relay)

```
stream-sound recv  [--port 47800] [--latency 20] [--forward IP[:PORT],..] [--no-play]
stream-sound send  --to IP[:PORT],.. [--source system|tone|app:NAME|input:NAME]
stream-sound sources     # list capturable sources
stream-sound peers       # list devices on the LAN
stream-sound update      # download and install the newest release
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
    app's streams to a private sink, which is removed on stop. libpulse is
    loaded at run time, so the binary links nothing but glibc.
- **Updates:** CI tags each release `v0.1.<run number>` and bakes the same
  number into the apps (`SSND_BUILD`, Android `versionCode`). The apps read
  `releases/latest` from the GitHub API and download the matching asset.

## Tested here (Linux, 2026-09-29)

- Tone and per-app (paplay) capture streamed and recorded back from the speaker
  output: correct pitch, zero clicks or dropouts.
- One sender to two receivers, two senders mixed on one receiver, and a
  two-hop daisy-chain: 125 s soak, 25,000 packets, 0 lost, 0 underruns.
- Measured path latency is roughly 5 ms capture + 5 ms packet + the buffer
  setting (20 ms default) + ~15 ms speaker buffer.

## Build from source

```
cargo build --release                                   # Linux (needs libxkbcommon-dev, libwayland-dev)
cargo build --release --target x86_64-pc-windows-gnu    # Windows (needs mingw-w64)
cargo test -p ssnd-core
```

Font: Loma from TLWG (GPL with font exception).
