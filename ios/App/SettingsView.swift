import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var model: AppModel
    @State private var name = ""
    @State private var newIP = ""
    @FocusState private var nameFocused: Bool

    private let modes: [(String, String, String)] = [
        ("game", "เกม", "หน่วงต่ำสุด เสียงตรงกับภาพที่สุด ถ้า Wi‑Fi สะดุดอาจได้ยินเสียงแตกสั้น ๆ"),
        ("balanced", "สมดุล", "หน่วงต่ำและไม่สะดุด ใช้ได้ทั่วไป (แนะนำ)"),
        ("music", "ฟังเพลง", "หน่วงมากขึ้น แต่ทน Wi‑Fi ที่ไม่เสถียรได้ดีที่สุด"),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.gap) {
            SectionTitle(text: "โหมดความหน่วง")
            ForEach(modes.indices, id: \.self) { i in
                modeCard(modes[i])
            }
            Text("คุณภาพเสียงเท่ากันทุกโหมด (ไม่บีบอัด) ต่างกันแค่ความหน่วงกับความทนต่อ Wi‑Fi สะดุด")
                .font(.system(size: 12)).foregroundColor(Theme.text3).padding(.horizontal, 4)

            SectionTitle(text: "เครื่องนี้")
            Card {
                Text("ชื่อที่เครื่องอื่นเห็น").font(.system(size: 13)).foregroundColor(Theme.text2)
                TextField("ชื่อเครื่อง", text: $name)
                    .focused($nameFocused)
                    .submitLabel(.done)
                    .onSubmit { model.rename(name) }
                    .onChange(of: nameFocused) { f in if !f { model.rename(name) } }
                    .textFieldStyle(FieldStyle())
                RowDivider()
                Text("IP ของเครื่องนี้ (แตะเพื่อคัดลอก)").font(.system(size: 13)).foregroundColor(Theme.text2)
                if model.state.ips.isEmpty {
                    Text("ไม่ได้ต่อ Wi‑Fi").font(.system(size: 15)).foregroundColor(Theme.text3)
                }
                ForEach(model.state.ips, id: \.self) { ip in
                    Button { model.copy(ip) } label: {
                        HStack {
                            Text(ip).font(.system(size: 16, weight: .medium).monospacedDigit()).foregroundColor(Theme.text)
                            Spacer()
                            Image(systemName: "doc.on.doc").foregroundColor(Theme.text3)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(PressStyle())
                }
            }

            SectionTitle(text: "เพิ่มเครื่องด้วย IP")
            Card {
                HStack(spacing: 8) {
                    TextField("เช่น 192.168.1.20", text: $newIP)
                        .keyboardType(.decimalPad)
                        .textFieldStyle(FieldStyle())
                    Button {
                        if model.addManual(newIP) { newIP = "" }
                    } label: {
                        Text("เพิ่ม").font(.system(size: 15, weight: .bold)).foregroundColor(.white)
                            .frame(width: 64, height: 44)
                            .background(Theme.red, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    }
                    .buttonStyle(PressStyle())
                }
                ForEach(model.manualIPs, id: \.self) { ip in
                    HStack {
                        Text(ip).font(.system(size: 15).monospacedDigit()).foregroundColor(Theme.text)
                        Spacer()
                        Button("ลบ") { model.removeManual(ip) }
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundColor(Theme.red)
                    }
                }
                Text("ใช้เมื่อเครื่องไม่ขึ้นเอง เช่น อยู่คนละวง Wi‑Fi")
                    .font(.system(size: 12)).foregroundColor(Theme.text3)
            }

            SectionTitle(text: "การแสดงผล")
            Card {
                Chips(options: [(VizStyle.bars, "ความถี่"), (VizStyle.wave, "คลื่นเสียง")], selection: $model.vizStyle)
                RowDivider()
                SwitchRow(title: "ให้จอเปิดค้างระหว่างสตรีม", subtitle: "จอดับแล้ว Wi‑Fi จะประหยัดพลังงาน ทำให้หน่วงเพิ่ม", isOn: $model.keepAwake)
                RowDivider()
                SwitchRow(title: "เปิดรับเสียงอัตโนมัติเมื่อเปิดแอป", isOn: $model.autoReceive)
            }

            SectionTitle(text: "อัปเดต")
            Card {
                HStack {
                    Text("เวอร์ชันนี้: \(Updater.currentText)").font(.system(size: 15)).foregroundColor(Theme.text)
                    Spacer()
                    if model.checkingUpdate { ProgressView().tint(Theme.text2) }
                }
                if !model.updateText.isEmpty {
                    Text(model.updateText).font(.system(size: 13)).foregroundColor(Theme.text2)
                }
                HStack(spacing: 8) {
                    Button {
                        Task { await model.checkUpdate(quiet: false) }
                    } label: {
                        pill("ตรวจสอบอัปเดต")
                    }
                    Button {
                        model.installUpdate(addSource: true)
                    } label: {
                        pill("เพิ่มแหล่งใน SideStore")
                    }
                }
                .buttonStyle(PressStyle())
                .disabled(model.checkingUpdate)
                RowDivider()
                SwitchRow(title: "รับเวอร์ชันเบต้าด้วย", subtitle: "ได้ของใหม่ก่อน แต่อาจยังมีจุดที่ไม่เสถียร", isOn: $model.allowBeta)
                Text("SideStore จะแจ้งและติดตั้งเวอร์ชันหลักให้ รวมถึงต่ออายุแอปทุก 7 วัน")
                    .font(.system(size: 12)).foregroundColor(Theme.text3)
            }

            ActionButton(title: "ปิดแอปและหยุดทั้งหมด", stop: true) { model.quit() }
                .padding(.top, 8)
        }
        .padding(.top, 4)
        .onAppear { name = model.state.name }
    }

    private func pill(_ t: String) -> some View {
        Text(t).font(.system(size: 14, weight: .semibold)).foregroundColor(Theme.text)
            .padding(.horizontal, 14).frame(height: 40)
            .background(Theme.surface2, in: Capsule())
    }

    private func modeCard(_ m: (String, String, String)) -> some View {
        let on = model.mode == m.0
        return Button {
            model.mode = m.0
        } label: {
            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    Circle().stroke(on ? Theme.red : Theme.outline, lineWidth: 2).frame(width: 20, height: 20)
                    if on { Circle().fill(Theme.red).frame(width: 10, height: 10) }
                }
                .padding(.top, 2)
                VStack(alignment: .leading, spacing: 3) {
                    Text(m.1).font(.system(size: 16, weight: .semibold)).foregroundColor(Theme.text)
                    Text(m.2).font(.system(size: 13)).foregroundColor(Theme.text2).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.surface, in: RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous)
                .stroke(on ? Theme.red : Theme.outline, lineWidth: on ? 2 : 1))
        }
        .buttonStyle(PressStyle())
    }
}

struct FieldStyle: TextFieldStyle {
    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration
            .font(.system(size: 16))
            .foregroundColor(Theme.text)
            .padding(.horizontal, 12)
            .frame(height: 44)
            .background(Theme.surface2, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.outline, lineWidth: 1))
    }
}
