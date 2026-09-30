import SwiftUI

struct ReceiveView: View {
    @EnvironmentObject var model: AppModel
    @State private var expanded: UInt32?
    @State private var showForward = false
    @State private var volumeOpen: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.gap) {
            hero
            SectionTitle(text: "กำลังรับจาก")
            Card {
                if !model.state.receiving {
                    Text("เปิดรับเสียงด้านบนก่อน").font(.system(size: 14)).foregroundColor(Theme.text3)
                } else if model.state.streams.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("รอเสียงจากเครื่องอื่น…").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
                        Text("เลือก \"\(model.state.name)\" ในแอป Stream Sound บนเครื่องที่จะส่ง แล้วกดเริ่มส่งเสียง")
                            .font(.system(size: 13)).foregroundColor(Theme.text2)
                    }
                } else {
                    ForEach(Array(model.state.streams.enumerated()), id: \.element.id) { i, s in
                        if i > 0 { RowDivider() }
                        streamRow(s)
                    }
                    Text("ไม่รวมเวลาเดินทางใน Wi‑Fi (~2–10 ms)").font(.system(size: 11)).foregroundColor(Theme.text3)
                }
            }
            forwardSection
        }
        .padding(.top, 4)
    }

    private var hero: some View {
        Card {
            HStack {
                if !model.state.receiving {
                    StatusPill(text: "ปิดรับอยู่", color: Theme.text3, dot: false)
                } else if model.state.streams.isEmpty {
                    StatusPill(text: "รอเสียง…", color: Theme.text2, dot: false)
                } else {
                    StatusPill(text: "กำลังเล่น", color: Theme.green)
                }
                Spacer()
                Toggle("", isOn: Binding(get: { model.state.receiving }, set: { model.setReceiving($0) }))
                    .labelsHidden()
                    .tint(Theme.red)
            }
            Visualizer(send: false, active: model.state.receiving && model.tab == .receive)
            VolumeTube()
            if !model.state.receiverError.isEmpty {
                Text(model.state.receiverError).font(.system(size: 12)).foregroundColor(Theme.error)
            }
        }
    }

    private func delay(_ s: StreamInfo) -> Double { s.captureMs + s.bufferMs + model.state.outputMs }

    private func streamRow(_ s: StreamInfo) -> some View {
        let open = expanded == s.id
        let warn = model.recentlyStuttered(s)
        return VStack(alignment: .leading, spacing: 8) {
            Button {
                withAnimation(.easeOut(duration: 0.2)) { expanded = open ? nil : s.id }
            } label: {
                HStack(spacing: 12) {
                    Avatar(name: s.name)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(s.name).font(.system(size: 16, weight: .medium)).foregroundColor(Theme.text).lineLimit(1)
                        Text(s.from).font(.system(size: 13).monospacedDigit()).foregroundColor(Theme.text2)
                    }
                    Spacer(minLength: 8)
                    speakerButton(s)
                    Text("\(Int(delay(s).rounded())) ms")
                        .font(.system(size: 13, weight: .semibold).monospacedDigit())
                        .foregroundColor(warn ? Theme.amber : Theme.green)
                        .padding(.horizontal, 10).padding(.vertical, 5)
                        .background((warn ? Theme.amber : Theme.green).opacity(0.14), in: Capsule())
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(PressStyle())
            if volumeOpen == s.from {
                sourceVolume(s)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
            if open {
                VStack(alignment: .leading, spacing: 4) {
                    Text(breakdown(s)).font(.system(size: 12).monospacedDigit()).foregroundColor(Theme.text2)
                    Text("หาย \(s.lost) · มาช้า \(s.late) · สะดุด \(s.underruns) · \(String(format: "%.1f", Double(s.rate) / 1000)) kHz")
                        .font(.system(size: 12).monospacedDigit()).foregroundColor(Theme.text3)
                }
                .padding(.leading, 50)
            }
        }
        .padding(.vertical, 4)
    }

    /// Tap to show this sender's own volume tube under its row.
    private func speakerButton(_ s: StreamInfo) -> some View {
        let v = model.sourceVolume(s.from)
        let on = volumeOpen == s.from
        return Button {
            withAnimation(.easeOut(duration: 0.2)) { volumeOpen = on ? nil : s.from }
        } label: {
            Image(systemName: v == 0 ? "speaker.slash.fill" : v < 0.6 ? "speaker.wave.1.fill" : "speaker.wave.3.fill")
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(on ? Theme.red : v == 1 ? Theme.text2 : Theme.text)
                .frame(width: 32, height: 32)
                .background(Theme.surface2, in: Circle())
        }
        .buttonStyle(PressStyle())
        .accessibilityLabel("ระดับเสียงของ \(s.name)")
    }

    private func sourceVolume(_ s: StreamInfo) -> some View {
        let v = model.sourceVolume(s.from)
        return HStack(spacing: 10) {
            Tube(value: Binding(get: { model.sourceVolume(s.from) }, set: { model.setSourceVolume(s.from, $0) }),
                 level: Double(s.level), height: 22, knobD: 20)
                .accessibilityLabel("ระดับเสียงของ \(s.name)")
            Text("\(Int((v * 100).rounded()))%")
                .font(.system(size: 13, weight: .semibold).monospacedDigit())
                .foregroundColor(Theme.text)
                .frame(width: 44, alignment: .trailing)
        }
        .padding(.leading, 50)
    }

    private func breakdown(_ s: StreamInfo) -> String {
        var parts: [String] = []
        if s.captureMs > 0 { parts.append("จับเสียง \(Int(s.captureMs))") }
        parts.append("บัฟเฟอร์ \(Int(s.bufferMs))")
        if model.state.outputMs > 0 { parts.append("ลำโพง \(Int(model.state.outputMs))") }
        return "หน่วงรวม ~\(Int(delay(s))) ms (" + parts.joined(separator: " + ") + ")"
    }

    private var forwardSection: some View {
        VStack(alignment: .leading, spacing: Theme.gap) {
            Button {
                withAnimation(.easeOut(duration: 0.2)) { showForward.toggle() }
            } label: {
                HStack {
                    Text("ส่งต่อ (ต่อเป็นทอด)").font(.system(size: 13, weight: .semibold)).foregroundColor(Theme.text2)
                    if !model.forwardTargets.isEmpty {
                        Text("\(model.forwardTargets.count) เครื่อง").font(.system(size: 12)).foregroundColor(Theme.red)
                    }
                    Spacer()
                    Image(systemName: "chevron.down")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(Theme.text3)
                        .rotationEffect(.degrees(showForward ? 180 : 0))
                }
                .padding(.top, 8).padding(.horizontal, 4)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if showForward {
                Card {
                    SwitchRow(title: "เล่นเสียงที่เครื่องนี้", subtitle: "ปิดไว้ถ้าอยากให้เครื่องนี้เป็นแค่ตัวส่งต่อ", isOn: $model.playLocal)
                    RowDivider()
                    Text("ส่งเสียงที่รับได้ต่อไปที่").font(.system(size: 13)).foregroundColor(Theme.text2)
                    let devices = model.devices
                    if devices.isEmpty {
                        EmptyDevices()
                    } else {
                        ForEach(Array(devices.enumerated()), id: \.element.id) { i, dev in
                            if i > 0 { RowDivider() }
                            DeviceRow(device: dev, checked: model.forwardTargets.contains(dev.ip)) {
                                model.toggleForward(dev.ip)
                            }
                        }
                    }
                    if model.state.forwarded > 0 {
                        Text("ส่งต่อแล้ว \(model.state.forwarded) แพ็กเก็ต").font(.system(size: 12)).foregroundColor(Theme.text3)
                    }
                }
            }
        }
    }
}
