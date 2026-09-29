import SwiftUI

struct Card<Content: View>: View {
    var padding: CGFloat = 16
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 12) { content }
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.surface, in: RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous).stroke(Theme.outline, lineWidth: 1))
    }
}

struct SectionTitle: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.system(size: 13, weight: .semibold))
            .foregroundColor(Theme.text2)
            .padding(.top, 8)
            .padding(.leading, 4)
    }
}

struct StatusPill: View {
    let text: String
    let color: Color
    var dot = true

    var body: some View {
        HStack(spacing: 6) {
            if dot { Circle().fill(color).frame(width: 7, height: 7) }
            Text(text).font(.system(size: 13, weight: .semibold)).foregroundColor(color)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(color.opacity(0.14), in: Capsule())
    }
}

/// Big full-width action: red gradient to start, deep red to stop.
struct ActionLabel: View {
    let title: String
    let stop: Bool

    var body: some View {
        Text(title)
            .font(.system(size: 17, weight: .bold))
            .foregroundColor(.white)
            .frame(maxWidth: .infinity, minHeight: 52)
            .background(
                RoundedRectangle(cornerRadius: Theme.buttonRadius, style: .continuous)
                    .fill(stop ? AnyShapeStyle(Theme.redDeep) : AnyShapeStyle(Theme.redGradient))
            )
            .shadow(color: stop ? .clear : Theme.red.opacity(0.35), radius: 12, y: 4)
    }
}

struct ActionButton: View {
    let title: String
    let stop: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) { ActionLabel(title: title, stop: stop) }
            .buttonStyle(PressStyle())
    }
}

struct PressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .opacity(configuration.isPressed ? 0.85 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

/// Segmented chips ("ทั้งเครื่อง / ไมโครโฟน", "ความถี่ / คลื่นเสียง").
struct Chips<T: Hashable>: View {
    let options: [(T, String)]
    @Binding var selection: T
    var disabled = false

    var body: some View {
        HStack(spacing: 6) {
            ForEach(options.indices, id: \.self) { i in
                let opt = options[i]
                let on = opt.0 == selection
                Button {
                    selection = opt.0
                } label: {
                    Text(opt.1)
                        .font(.system(size: 14, weight: on ? .semibold : .regular))
                        .foregroundColor(on ? .white : Theme.text2)
                        .frame(maxWidth: .infinity, minHeight: 38)
                        .background(on ? Theme.red : Theme.surface2, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }
                .buttonStyle(PressStyle())
            }
        }
        .disabled(disabled)
        .opacity(disabled ? 0.5 : 1)
    }
}

struct Avatar: View {
    let name: String
    var body: some View {
        Text(String(name.trimmingCharacters(in: .whitespaces).prefix(1)).uppercased())
            .font(.system(size: 16, weight: .bold))
            .foregroundColor(Theme.text)
            .frame(width: 38, height: 38)
            .background(Theme.surface2, in: Circle())
    }
}

/// A device with a tick circle on the right.
struct DeviceRow: View {
    let device: DeviceItem
    let checked: Bool
    let toggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            HStack(spacing: 12) {
                Avatar(name: device.name)
                VStack(alignment: .leading, spacing: 2) {
                    Text(device.name).font(.system(size: 16, weight: .medium)).foregroundColor(Theme.text).lineLimit(1)
                    HStack(spacing: 6) {
                        Text(device.ip).font(.system(size: 13).monospacedDigit()).foregroundColor(Theme.text2)
                        if !device.receiving {
                            Text("ไม่ได้เปิดรับ").font(.system(size: 13)).foregroundColor(Theme.text3)
                        } else if device.manual {
                            Text("เพิ่มด้วย IP").font(.system(size: 13)).foregroundColor(Theme.text3)
                        }
                    }
                }
                Spacer(minLength: 8)
                ZStack {
                    Circle().stroke(checked ? Theme.red : Theme.outline, lineWidth: 2).frame(width: 24, height: 24)
                    if checked {
                        Circle().fill(Theme.red).frame(width: 24, height: 24)
                        Image(systemName: "checkmark").font(.system(size: 12, weight: .bold)).foregroundColor(.white)
                    }
                }
            }
            .padding(.vertical, 6)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

struct EmptyDevices: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("ยังไม่พบเครื่องอื่น").font(.system(size: 15, weight: .semibold)).foregroundColor(Theme.text)
            Text("เปิด Stream Sound บนอีกเครื่องใน Wi‑Fi เดียวกัน แล้วรอสักครู่ หรือเพิ่มด้วย IP ในหน้าตั้งค่า")
                .font(.system(size: 13)).foregroundColor(Theme.text2)
            Text("ถ้ายังไม่ขึ้น: ตั้งค่า iPhone > ความเป็นส่วนตัวและความปลอดภัย > เครือข่ายเฉพาะที่ > เปิด Stream Sound")
                .font(.system(size: 12)).foregroundColor(Theme.text3)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct SwitchRow: View {
    let title: String
    var subtitle: String? = nil
    @Binding var isOn: Bool

    var body: some View {
        Toggle(isOn: $isOn) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 15)).foregroundColor(Theme.text)
                if let s = subtitle {
                    Text(s).font(.system(size: 12)).foregroundColor(Theme.text3)
                }
            }
        }
        .tint(Theme.red)
    }
}

/// Divider between rows inside a card.
struct RowDivider: View {
    var body: some View { Rectangle().fill(Theme.outline).frame(height: 1) }
}
