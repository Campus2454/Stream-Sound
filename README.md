# Stream Sound

Low-latency audio streaming between devices on the same LAN. One Rust engine
shared by every platform; desktop UI in Thai.

## Builds

Every push builds Windows, Linux and Android automatically, and every change
on `main` is published as a release that installed apps update from. See
**การติดตั้ง** below for where to download them and how to install.

| Platform | Status |
|---|---|
| Windows 10/11 | Built on GitHub's Windows runner and used on Windows 11; installer tested in Wine |
| Linux (PulseAudio or PipeWire) | Built and tested end to end; one standalone file |
| Android 10+ | Built on GitHub; not yet tried on a real phone |
| iOS | Not built yet (planned: cloud Mac build, sideloaded from Windows) |

## การติดตั้ง

### ดาวน์โหลดไฟล์

เปิด <https://github.com/Campus2454/Stream-Sound/releases/latest>
(ไม่ต้องล็อกอิน) แล้วเลื่อนลงไปที่ **Assets**:

| เครื่อง | ไฟล์ |
|---|---|
| Windows | `StreamSound-windows-x64-setup.exe` |
| Linux | `StreamSound-linux-x86_64` |
| Android | `StreamSound.apk` |

ติดตั้งครั้งแรกครั้งเดียว หลังจากนั้นแอปจะอัปเดตตัวเองจากหน้านี้ (ดู **อัปเดตอัตโนมัติ** ด้านล่าง)

ไฟล์ทดสอบที่ยังไม่รวมเข้า `main` อยู่ที่ <https://github.com/Campus2454/Stream-Sound/actions>
(ต้องล็อกอิน): กดรายการที่มี **✓ สีเขียว** แล้วดาวน์โหลดจากหัวข้อ **Artifacts** (ได้เป็นไฟล์ .zip)

### Windows 10/11

1. เปิด `StreamSound-windows-x64-setup.exe`
   - ถ้าขึ้น "Windows protected your PC" ให้กด **More info** > **Run anyway**
   - Windows จะถามสิทธิ์ผู้ดูแล ให้กด **Yes** (ติดตั้งลง `Program Files`)
2. เลือกโฟลเดอร์ (ค่าเริ่มต้น `C:\Program Files\Stream Sound`) และทางลัดที่ต้องการ
   (บนเดสก์ท็อป / ในเมนู Start) แล้วกด **ติดตั้ง**
3. หน้าสุดท้ายติ๊ก **เปิด Stream Sound** ไว้ แล้วกด **เสร็จสิ้น**
4. ถ้า Windows Firewall ถาม ให้ติ๊ก **Private networks** แล้วกด **Allow**
   (ถ้าไม่อนุญาต เครื่องอื่นจะส่งเสียงมาเครื่องนี้ไม่ได้)

ถอนการติดตั้ง: **การตั้งค่า > แอป > Stream Sound > ถอนการติดตั้ง** หรือในแอปที่หน้า
**ตั้งค่า** > **ถอนการติดตั้ง…** เลือกได้ว่าจะลบการตั้งค่าด้วยหรือไม่

เคยใช้ `StreamSound.exe` แบบไม่ติดตั้ง (v1.0.4 หรือก่อนหน้า): ติดตั้งด้วยไฟล์ setup ด้านบนครั้งเดียว
แล้วลบไฟล์ `.exe` เดิมทิ้งได้ (v1.0.1–v1.0.4 อัปเดตตัวเองไม่ได้เพราะแอปค้างตอนปิด ซึ่งแก้แล้ว)
ถ้าต้องการใช้แบบไม่ติดตั้ง ไฟล์ `StreamSound-windows-x64.zip` ยังมีให้ในทุก release

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

1. ทำให้ไฟล์เปิดได้ แล้วเปิด (หรือคลิกขวาไฟล์ > Properties > อนุญาตให้รันเป็นโปรแกรม แล้วดับเบิลคลิก):
   ```
   chmod +x ~/Downloads/StreamSound-linux-x86_64
   ~/Downloads/StreamSound-linux-x86_64
   ```
   ไฟล์นี้ติดตั้งตัวเอง: เลือกโฟลเดอร์ (ค่าเริ่มต้น `~/.local/share/stream-sound`)
   ทางลัดบนเดสก์ท็อป และเมนูแอป แล้วกด **ติดตั้ง** ไม่ต้องใช้ sudo
   (กด **ใช้เลยโดยไม่ติดตั้ง** ถ้าต้องการเปิดจากไฟล์นี้ตรง ๆ)
   ถอนการติดตั้ง: คลิกขวาไอคอนในเมนูแอป > **ถอนการติดตั้ง Stream Sound**
   หรือในแอปที่หน้า **ตั้งค่า** > **ถอนการติดตั้ง…** (หรือ `stream-sound --uninstall`)
2. ถ้าขึ้นว่า "ไม่พบ PulseAudio/PipeWire (libpulse)": เครื่องที่ใช้ PipeWire ต้องมี
   `pipewire-pulse` ด้วย: `sudo pacman -S --needed libpulse pipewire-pulse`
   (ตรวจได้ด้วย `pactl info` ถ้าขึ้น `Server Name` แสดงว่าใช้ได้)
3. การส่งเสียงเฉพาะแอปใช้คำสั่ง `pactl` ซึ่งมากับแพ็กเกจ `libpulse` บน Arch อยู่แล้ว
4. ถ้าเปิดไฟร์วอลล์ไว้ ให้เปิดพอร์ต UDP 47800–47801
   (ufw: `sudo ufw allow 47800:47801/udp`,
   firewalld: `sudo firewall-cmd --permanent --add-port=47800-47801/udp && sudo firewall-cmd --reload`)

### อัปเดตอัตโนมัติ

เวอร์ชันมี 2 แบบ และทุกเวอร์ชันเก็บไว้ในหน้า [Releases](https://github.com/Campus2454/Stream-Sound/releases) ไม่ถูกลบ

- **`vX.Y` เวอร์ชันทางการ** (เช่น v0.2, v1.0) ออกเมื่อสั่ง: ไปที่ Actions > build > **Run workflow**
  เลือก branch `main` ติ๊ก **official** แล้วกด Run (ช่อง version เว้นว่างได้ จะได้เลขถัดไปให้เอง)
- **`vX.Y.Z` เวอร์ชันเบต้า** ออกเองทุกครั้งที่มีการแก้โค้ดเข้า `main` เช่นหลัง v0.2 จะเป็น v0.2.1, v0.2.2, …
  (ก่อนมีเวอร์ชันทางการครั้งแรก เบต้าจะใช้เลขต่อจากของเดิม v0.1.14, v0.1.17, …)
- ลำดับใหม่กว่า: v0.2 → v0.2.1 → v0.2.2 → v0.3
- แอปรับเวอร์ชันเบต้าด้วยเป็นค่าเริ่มต้น ปิดได้ที่หน้า **ตั้งค่า** > **รับเวอร์ชันเบต้าด้วย**
  (ปิดแล้วจะอัปเดตเฉพาะเวอร์ชันทางการ)

- **Windows / Linux:** แอปเช็กตอนเปิดและทุก 5 นาทีขณะเปิดแอปอยู่ ถ้ามีเวอร์ชันใหม่จะดาวน์โหลดเก็บไว้เงียบ ๆ
  แล้วขึ้นแถบสีแดง "มีเวอร์ชันใหม่" กด **อัปเดตเลย** เมื่อสะดวก (แอปไม่รีสตาร์ตเองกลางการสตรีม)
  แอปจะปิดทันที แล้วตัวติดตั้งจะอัปเดตโดยขึ้นหน้าต่างความคืบหน้าเล็ก ๆ และเปิดเวอร์ชันใหม่ให้เอง
  (Windows จะถามสิทธิ์ผู้ดูแลทุกครั้ง ให้กด **Yes**)
  เลขเวอร์ชันปัจจุบันและปุ่ม **ตรวจสอบอัปเดต** อยู่ที่หน้า **ตั้งค่า**
- **Android:** แอปเช็กและดาวน์โหลดตอนเปิดและทุก 5 นาทีขณะเปิดแอปอยู่ แล้วขึ้นแถบ **อัปเดตเลย**
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
   - ปรับเสียงโดยลากปุ่มกลมบนแถบระดับเสียง แถบจะสว่างขึ้นตามเสียงที่เข้ามา ดังได้ถึง 200%:
     3 ส่วน 4 แรกของแถบคือ 0–100% (ขีดเล็ก ๆ คือ 100%) ส่วนที่เหลือคือ 101–200%
     แตะรูปลำโพงเพื่อปิดเสียง (หมุนล้อเมาส์บนแถบจะไม่เปลี่ยนเสียง กันเสียงดังขึ้นโดยไม่ตั้งใจ)
   - "กำลังรับจาก" แสดงทุกเครื่องที่ส่งมา พร้อมความหน่วงรวมเป็น ms (สีเขียว = ลื่น, สีเหลือง = เพิ่งสะดุด)
     แตะที่รายการเพื่อดูว่าหน่วงตรงไหน (จับเสียง + บัฟเฟอร์ + ลำโพง) และจำนวนเสียงที่หาย/มาช้า
   - ปรับเสียงแยกแต่ละเครื่องที่ส่งมาได้: บนคอมเอาเมาส์ชี้รูปลำโพงเล็กหน้าตัวเลข ms แล้วแถบปรับเสียงจะโผล่ขึ้นมา
     (คลิกลำโพงเพื่อปิดเสียงเฉพาะเครื่องนั้น) บนมือถือแตะรูปลำโพง แอปจำระดับเสียงของแต่ละเครื่องไว้
3. **ส่งเสียง:** เลือกแหล่งเสียง (ทั้งเครื่อง / เฉพาะแอป / อินพุตหรือไมโครโฟน / เสียงทดสอบบนคอม)
   ติ๊กเครื่องปลายทางได้หลายเครื่อง แล้วกด **เริ่มส่งเสียง**
4. **ส่งต่อเป็นทอด:** บนเครื่องกลาง หน้า **รับเสียง > ส่งต่อ** ติ๊กเครื่องถัดไป
   (ปิด "เล่นเสียงที่เครื่องนี้" ได้ถ้าไม่อยากให้เครื่องกลางดังด้วย)
5. **หลายเครื่องส่งมาเครื่องเดียว:** ให้ทุกเครื่องส่งมาที่เครื่องเดียวกัน เสียงจะถูกมิกซ์รวมกันเอง
6. ภาพเสียงในหน้าส่งและรับมี 2 แบบ: **แท่งความถี่** และ **คลื่นเสียง** (2 วินาทีล่าสุด)
7. **บนคอม (Windows / Linux):** กดปิดหน้าต่างแล้วแอปจะไปอยู่ใน **ถาดข้างนาฬิกา** และยังรับส่งเสียงต่อ
   คลิกไอคอนเพื่อเปิดหน้าต่างอีกครั้ง คลิกขวา > **ออก** เพื่อปิดแอปจริง ๆ
   (เปิดแอปซ้ำขณะที่อยู่ในถาด จะเป็นการเปิดหน้าต่างเดิมขึ้นมา)
   - เปิด **ตั้งค่า > ทั่วไป > เปิดพร้อมเครื่อง** เพื่อให้แอปเปิดเองในถาดตอนเปิดเครื่อง
     สวิตช์นี้ตรงกับรายการใน Task Manager > Startup apps (Windows) หรือ Autostart ของเดสก์ท็อป (Linux)
     ปิดจากที่ไหนก็ได้ผลเหมือนกัน
   - Linux: ถาดใช้ได้บน KDE, Xfce, Cinnamon และ GNOME ที่ติดตั้งส่วนขยาย AppIndicator
     (`sudo pacman -S gnome-shell-extension-appindicator` แล้วเปิดในแอป Extensions)
     ถ้าเดสก์ท็อปไม่มีถาด ปิดหน้าต่างจะเป็นการปิดแอปเหมือนเดิม
     แอปเพิ่มตัวเองพร้อมไอคอนในเมนูแอปของเดสก์ท็อปให้เองตอนเปิดครั้งแรก
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
- **Updates:** releases are tagged `vX.Y` (official, run by hand) or `vX.Y.Z`
  (beta, every push to main; Z counts the changes since vX.Y), worked out by
  `.github/version.sh`. Betas are GitHub pre-releases once an official release
  exists. CI bakes the version into the apps (`SSND_VERSION`, Android
  `versionName`, and `versionCode` = X·10⁷ + Y·10⁵ + Z). The apps check at
  launch and every 5 minutes while running, through github.com pages rather
  than the REST API (which allows 60 anonymous requests an hour per IP): with
  betas on they read the tags in `releases.atom` (the latest 10 releases), with
  betas off the tag `releases/latest` redirects to. They then download
  `releases/download/<tag>/<file>`; a 404 there means the release's files are
  still uploading, and the next check tries again.
- **Installers:** Windows has an NSIS setup (`desktop/installer/windows.nsi`,
  built in CI): per-machine install, shortcuts, an entry in Settings > Apps
  and `uninstall.exe`. The updater downloads the new setup to
  `%TEMP%\StreamSound` and runs it with `/UPDATE /D=<folder>` (progress only,
  then it reopens the app); a copy that isn't installed runs the full setup
  with `/FROM="<exe>"`, which removes that file once installed. On Linux the
  single file installs itself (`desktop/src/setup/linux.rs`, window in
  `gui/setup.rs`): the program, a menu entry with an Uninstall action, an
  optional desktop shortcut and the icon, recorded in
  `~/.config/StreamSound/install.txt`; updates run the new file with
  `--update <installed program>`. Before replacing files, both ask the
  running app to quit over its single-instance port (`SSND quit`; the
  program's `--quit` flag). Releases still carry `StreamSound-windows-x64.exe`
  for versions before the installer, which download that name.

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
