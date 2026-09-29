# Stream Sound

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
  แล้วขึ้นแถบสีแดง "มีเวอร์ชันใหม่" กด **อัปเดตเลย** เมื่อสะดวก (แอปไม่รีสตาร์ตเองกลางการสตรีม)
  เลข build ปัจจุบันและปุ่ม **ตรวจสอบอัปเดต** อยู่ที่หน้า **ตั้งค่า**
- **Android:** แอปเช็กและดาวน์โหลดตอนเปิด แล้วขึ้นแถบ **อัปเดตเลย**
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

แอปมี 3 หน้า: **ส่งเสียง**, **รับเสียง** และ **ตั้งค่า** (บนคอมอยู่ใต้หัวแอป บนมือถืออยู่ด้านล่างจอ)
ชื่อและ IP ของเครื่องนี้อยู่มุมขวาบน แตะเพื่อคัดลอก IP

1. เปิดแอปบนทุกเครื่องที่ต่อ Wi-Fi/LAN วงเดียวกัน เครื่องอื่นจะขึ้นในรายการเอง
   ถ้าไม่ขึ้น ไปที่ **ตั้งค่า > เพิ่มเครื่องด้วย IP**
2. **รับเสียง:** เปิดรับอยู่แล้วตั้งแต่เปิดแอป (ปิดได้ที่สวิตช์ "เปิดรับ")
   - ปรับเสียงโดยลากปุ่มกลมบนแถบระดับเสียง แถบจะสว่างขึ้นตามเสียงที่เข้ามา ขีดเล็ก ๆ คือ 100% (ดังได้ถึง 150%)
     แตะรูปลำโพงเพื่อปิดเสียง
   - "กำลังรับจาก" แสดงทุกเครื่องที่ส่งมา พร้อมความหน่วงรวม (สีเขียว = ลื่น, สีเหลือง = เพิ่งสะดุด)
     แตะที่รายการเพื่อดูว่าหน่วงตรงไหน (จับเสียง + บัฟเฟอร์ + ลำโพง) และจำนวนเสียงที่หาย/มาช้า
3. **ส่งเสียง:** เลือกแหล่งเสียง (ทั้งเครื่อง / เฉพาะแอป / อินพุตหรือไมโครโฟน / เสียงทดสอบบนคอม)
   ติ๊กเครื่องปลายทางได้หลายเครื่อง แล้วกด **เริ่มส่งเสียง**
4. **ส่งต่อเป็นทอด:** บนเครื่องกลาง หน้า **รับเสียง > ส่งต่อ** ติ๊กเครื่องถัดไป
   (ปิด "เล่นเสียงที่เครื่องนี้" ได้ถ้าไม่อยากให้เครื่องกลางดังด้วย)
5. **หลายเครื่องส่งมาเครื่องเดียว:** ให้ทุกเครื่องส่งมาที่เครื่องเดียวกัน เสียงจะถูกมิกซ์รวมกันเอง
6. ภาพเสียงในหน้าส่งและรับมี 2 แบบ: **แท่งความถี่** และ **คลื่นเสียง** (2 วินาทีล่าสุด)
   แตะที่ภาพเพื่อสลับ หรือเลือกที่ **ตั้งค่า > ภาพเสียง**
7. **ตั้งค่า > โหมดเสียง** (ตั้งทั้งฝั่งส่งและฝั่งรับให้ตรงกันจะดีที่สุด):
   - **เกม (หน่วงต่ำสุด)**: เสียงตรงกับภาพที่สุด เสียงที่มาช้าจะถูกข้ามไป ถ้า Wi-Fi สะดุดอาจได้ยินเสียงแตกสั้น ๆ
   - **สมดุล** (ค่าเริ่มต้น): หน่วงต่ำและไม่สะดุด ใช้ได้ทั่วไป
   - **ฟังเพลง (เสถียรสุด)**: หน่วงมากขึ้น แต่ทน Wi-Fi ที่ไม่เสถียรได้ดีที่สุด

   คุณภาพเสียงเท่ากันทุกโหมด (PCM 16-bit ไม่บีบอัด) เพราะการบีบอัดเสียงจะเพิ่มความหน่วง
   ทุกโหมดปรับบัฟเฟอร์เองตามสภาพเครือข่าย
8. **ตั้งค่า > เครื่องนี้:** เปลี่ยนชื่อที่เครื่องอื่นเห็นได้ (เช่น "PC ห้องนั่งเล่น")
9. แอปจำทุกอย่างที่เลือกไว้ (เครื่องปลายทาง, แหล่งเสียง, ระดับเสียง, โหมด, IP ที่เพิ่มเอง) เปิดครั้งหน้าพร้อมใช้ทันที
10. มือถือ: **ตั้งค่า > ปิดแอปและหยุดทั้งหมด** ปิดทุกอย่างรวมถึงการทำงานเบื้องหลัง

### ความหน่วงที่ทำได้จริง

ความหน่วงระดับ "ไม่กี่มิลลิวินาที" ทำไม่ได้ทางกายภาพ: ลำโพงของระบบเองก็ใช้ 5–20 ms แล้ว
และ Wi-Fi มีช่วงสะดุด 5–30 ms เป็นปกติ ตัวเลขที่คาดได้ในโหมดเกม:

| เส้นทาง | หน่วงรวมโดยประมาณ |
|---|---|
| PC → PC ผ่านสาย LAN | 20–30 ms |
| PC → มือถือ ผ่าน Wi-Fi ที่ดี (แอปเปิดค้างบนจอ) | 40–60 ms |
| มือถือ → PC (จับเสียงบน Android เพิ่มเอง ~20–40 ms) | 60–90 ms |
| มือถือจอดับ (Wi-Fi เข้าโหมดประหยัดไฟ) | เพิ่มได้อีก ~100 ms |

เคล็ดลับ: ใช้สาย LAN ถ้าได้, ให้มือถืออยู่ใกล้เราเตอร์และใช้ 5 GHz, เปิดแอปค้างบนจอระหว่างเล่นเกม
(แอปจะไม่ให้จอดับเองระหว่างสตรีม)

## Command line (testing / headless relay)

```
stream-sound recv  [--port 47800] [--mode game|balanced|music] [--forward IP[:PORT],..] [--no-play]
stream-sound send  --to IP[:PORT],.. [--source system|tone|app:NAME|input:NAME] [--mode game|balanced|music]
stream-sound sources     # list capturable sources
stream-sound peers       # list devices on the LAN
stream-sound update      # download and install the newest release
```

## How it works

- **Transport:** UDP, port 47800 (audio) and 47801 (discovery broadcast).
  Uncompressed 16-bit PCM at the source's sample rate (≈1.5 Mbit/s for
  48 kHz stereo: lossless, no codec delay), 2.5 ms packets in game mode and
  5 ms otherwise. Each packet carries the sender's capture delay and a flag
  when the sender's audio resumes after silence, so a pause is never mistaken
  for a network glitch.
- **Receiver:** one jitter buffer per source. Every second it measures how far
  the buffer dipped below its average and sizes itself from the recent worst
  dips plus a small margin (2 ms game, 5 ms balanced, 15 ms music). Excess is
  removed by playing up to 2 % faster or, when large, by a 2.5 ms crossfade
  skip. Late or lost packets are waited for only until they are due, then
  concealed by repeating the last few milliseconds with a fade; in game mode
  audio that arrives after its time is dropped so sound stays in sync with
  the picture. Catmull-Rom resampling to the speaker's rate, then all sources
  mixed with a soft limiter.
- **Delay reporting:** capture delay (from PulseAudio, WASAPI or Android's
  AudioRecord timestamps), buffer, and speaker delay (PulseAudio latency, cpal
  timestamps, AAudio/AudioTrack timestamps) are shown separately.
- **Scheduling:** audio and network threads ask for higher priority
  (TIME_CRITICAL/HIGHEST on Windows; nice -19/-16 on Linux and Android where
  the system allows it).
- **Daisy-chain:** a relay forwards each packet the moment it arrives, before any
  buffering, so each hop adds well under 1 ms. A hop counter stops loops.
- **Stability:** capture and playback run in their own threads and reconnect by
  themselves when a device changes or disappears; panics inside the network
  loop are caught and the loop restarts.
- **Android:** the speaker is driven by AAudio from Rust (low-latency mode,
  exclusive MMAP in game mode where the phone supports it), starting at two
  hardware bursts of buffer and growing one burst per underrun. If AAudio
  can't open, the Kotlin app falls back to a low-latency `AudioTrack` tuned the
  same way. Capture is `AudioRecord` (playback capture or the mic) in the
  Kotlin app, pushed into the same Rust engine through JNI (`android/rust`).
  A foreground service with Wi-Fi low-latency and wake locks keeps it running
  with the screen off; the screen is kept on while streaming with the app
  open, because phones only drop Wi-Fi power saving with the screen on.
- **Capture:**
  - Windows: WASAPI loopback (whole system), process loopback (one app, needs
    Windows 10 2004+), any input device. System capture keeps a silent
    output stream open on the same device so loopback keeps delivering audio
    in real time even when nothing else is playing.
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
- Click test in game mode (packet sent until it leaves the sound server):
  19–26 ms, of which ~10–14 ms is the jitter buffer and ~12 ms the sound
  server's own output buffer. Balanced settles at ~30 ms buffer, music ~65 ms.
- Simulated Wi-Fi (2 ms base delay with jitter, 0.3 % loss, a 25 ms stall
  every ~4 s): game ~32 ms with occasional short concealments,
  balanced ~45 ms with none. Simulated phone Wi-Fi power save (packets only at
  102 ms beacons): balanced adapts to ~130 ms.

## Build from source

```
cargo build --release                                   # Linux (needs libxkbcommon-dev, libwayland-dev)
cargo build --release --target x86_64-pc-windows-gnu    # Windows (needs mingw-w64)
cargo test -p ssnd-core
```

Font: Loma from TLWG (GPL with font exception).
