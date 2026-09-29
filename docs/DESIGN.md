# Stream Sound design

One look on every platform (Windows/Linux desktop, Android, iOS). Thai UI,
red on near-black, content in three tabs.

## Colours

| Token | Hex | Use |
|---|---|---|
| `bg` | `#0A0A0C` | window / screen background |
| `surface` | `#141417` | cards |
| `surface2` | `#1D1D22` | tracks, inputs, unselected segments, avatars |
| `outline` | `#2A2A31` | card and input borders |
| `red` | `#E62639` | primary actions, selection, active tab |
| `redHi` | `#FF5A6A` | top of gradients, glow |
| `redDeep` | `#8C1320` | pressed, "stop" fills |
| `text` | `#F4F4F6` | primary text |
| `text2` | `#A3A3AD` | secondary text |
| `text3` | `#6E6E78` | hints, disabled |
| `green` | `#3DDC84` | live / connected / stable |
| `amber` | `#FFB020` | some stutter, warnings |
| `error` | `#FF6B6B` | errors |

Radii: cards 16, buttons 14, pills fully round. Spacing: 16 page padding,
12 between cards, 8 inside rows.

## Structure

Header (every tab): logo mark (red rounded square with five white equalizer
bars) + "Stream Sound"; on the right a device chip "● {name} · {ip}" (tap to
copy the IP).

Three tabs: **ส่งเสียง** (Send), **รับเสียง** (Receive), **ตั้งค่า**
(Settings). Desktop: a segmented bar under the header. Phones: a bottom
navigation bar. A red dot on Send while sending, a green dot on Receive
while audio is arriving.

An update-ready banner (red gradient card, "อัปเดตเลย") sits above the tab
content when a new build is downloaded. Messages appear as a toast pill at
the bottom for ~3 s.

### Send

1. Hero card: status pill ("● กำลังส่ง" red / "พร้อมส่ง"), capture delay on
   the right while sending, the visualizer (send tap), then one full-width
   button: "เริ่มส่งเสียง" (red gradient) or "หยุดส่ง" (redDeep).
2. "แหล่งเสียง": segmented chips. Desktop: ทั้งเครื่อง / เฉพาะแอป / อินพุต /
   เสียงทดสอบ, with a dropdown for the app or input. Android: ทั้งเครื่อง /
   เฉพาะแอป / ไมโครโฟน. iOS: ทั้งเครื่อง (screen broadcast) / ไมโครโฟน.
3. "ส่งไปที่": device rows (avatar with the name's first letter, name, IP,
   check circle on the right). A device that isn't receiving shows
   "ไม่ได้เปิดรับ" in text3. Empty state explains how to make devices appear.
   Changing ticks while sending applies immediately.

### Receive

1. Hero card: status pill ("● กำลังเล่น" green / "รอเสียง…" / "ปิดรับอยู่"),
   an on/off switch on the right, the visualizer (receive tap), then the
   **volume tube**: speaker button (mute) + tube + percentage.
2. "กำลังรับจาก": one row per incoming stream: name, sender IP, a delay pill
   ("32 ms", green when stable, amber after recent stutters). Tapping a row
   shows the breakdown (capture + buffer + speaker) and lost / late / stutter
   counts.
3. "ส่งต่อ (ต่อเป็นทอด)" (collapsed by default): "เล่นเสียงที่เครื่องนี้"
   switch and device rows to forward to.

### Settings

1. "โหมดเสียง": three stacked option cards (เกม / สมดุล / ฟังเพลง) with
   their one-line hint; the selected card has a red border and dot.
2. "เครื่องนี้": editable device name, this device's IPs (tap to copy).
3. "เพิ่มเครื่องด้วย IP": field + add button, list with remove.
4. "ทั่วไป": visualizer style แท่งความถี่ (bars) / คลื่นเสียง (waveform) as a
   segmented control, then switches: "เปิดรับเสียงทันทีเมื่อเปิดแอป" (all) and
   "ให้จอเปิดค้างระหว่างสตรีม" (phones).
5. "อัปเดต": build number, status line, "ตรวจสอบอัปเดต".
6. Phone only: "ปิดแอปและหยุดทั้งหมด" (outlined, error-coloured text).

Everything the user picks (targets, source, volume, mode, style, typed IPs,
name) is saved and restored on the next launch. Targets are remembered by
"ip:port"; a remembered target that isn't around shows "ไม่พบในเครือข่ายตอนนี้"
so it can still be unticked. An avatar for an address shows "IP".

## Visualizer

Engine API: `engine.scope(Tap::Send | Tap::Receive)` returns a snapshot with
`spectrum(bands)` (0..1 per log-spaced band, 30 Hz–16 kHz) and
`waveform(cols)` (0..1 peak per column over the last 2 s, oldest first).

- **ความถี่ (default)**: 48 bars (32 on phones), mirrored around a centre
  line: upper half full colour with a vertical gradient red → redHi, lower
  half the same at 30 % opacity. Bars rise instantly and fall at ~2.5 heights
  per second. Minimum bar height 2 px so silence shows a dotted line in text3.
- **คลื่นเสียง**: the 2-second peak envelope as thin mirrored bars scrolling
  right to left, same colours.
- Tapping the visualizer switches style. Poll at display rate only while the
  tab is visible and that side is active.

## Volume tube

A 28 px pill. Track `surface2`; a tick at 100 %. The knob (22 px white circle
with a red ring) sits at the volume (0–150 %). The live level fills the tube
from the left in a red → redHi gradient, scaled so it never passes the knob
(fill = level before volume × knob position): the sound "fills the tube up
to your volume". Drag or tap anywhere to set; scroll wheel on desktop.
