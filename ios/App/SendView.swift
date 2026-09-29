import ReplayKit
import SwiftUI

struct SendView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.gap) {
            hero
            SectionTitle(text: "แหล่งเสียง")
            Card {
                Chips(options: [(SendSource.screen, "ทั้งเครื่อง"), (SendSource.mic, "ไมโครโฟน")],
                      selection: $model.sendSource, disabled: model.sending)
                Text(model.sendSource == .screen
                     ? "ส่งทุกเสียงที่ iPhone เล่นอยู่ ผ่านการถ่ายทอดหน้าจอของ iOS (ส่งแค่เสียง) เสียงที่มีลิขสิทธิ์ เช่น Netflix หรือ Apple Music จะเงียบ"
                     : "ส่งเสียงจากไมโครโฟนของ iPhone")
                    .font(.system(size: 12))
                    .foregroundColor(Theme.text3)
            }
            SectionTitle(text: "ส่งไปที่")
            Card {
                let devices = model.devices
                if devices.isEmpty {
                    EmptyDevices()
                } else {
                    ForEach(Array(devices.enumerated()), id: \.element.id) { i, dev in
                        if i > 0 { RowDivider() }
                        DeviceRow(device: dev, checked: model.sendTargets.contains(dev.ip)) {
                            model.toggleSend(dev.ip)
                        }
                    }
                }
            }
        }
        .padding(.top, 4)
    }

    private var hero: some View {
        Card {
            HStack {
                if model.sending {
                    StatusPill(text: "กำลังส่ง", color: Theme.red)
                } else {
                    StatusPill(text: "พร้อมส่ง", color: Theme.text2, dot: false)
                }
                Spacer()
                if model.sending, model.captureMs > 0 {
                    Text("จับเสียง ~\(Int(model.captureMs)) ms")
                        .font(.system(size: 12).monospacedDigit())
                        .foregroundColor(Theme.text2)
                }
            }
            Visualizer(send: true, active: model.sending && model.tab == .send)
            if model.sendSource == .screen || model.broadcasting {
                BroadcastButton(live: model.broadcasting)
                    .frame(height: 52)
                    .simultaneousGesture(TapGesture().onEnded { model.pushConfig(force: true) })
                if model.broadcasting, model.sendTargets.isEmpty {
                    Text("ยังไม่ได้เลือกเครื่องปลายทาง เลือกด้านล่างได้เลย เสียงจะไปทันที")
                        .font(.system(size: 12)).foregroundColor(Theme.amber)
                } else if !model.broadcasting {
                    Text("กดแล้วเลือก Stream Sound > เริ่มการถ่ายทอด เสียงจะส่งต่อได้แม้ออกไปใช้แอปอื่น")
                        .font(.system(size: 12)).foregroundColor(Theme.text3)
                }
            } else {
                ActionButton(title: model.micSending ? "หยุดส่ง" : "เริ่มส่งเสียง", stop: model.micSending) {
                    if model.micSending { model.stopMic() } else { model.startMic() }
                }
            }
            if let err = errorText {
                Text(err).font(.system(size: 12)).foregroundColor(Theme.error)
            }
        }
    }

    private var errorText: String? {
        if let r = model.report, !r.error.isEmpty { return r.error }
        if model.micSending, !model.state.sendError.isEmpty { return model.state.sendError }
        return nil
    }
}

/// Our red button with iOS's broadcast picker laid invisibly over it: only
/// the system picker may start a screen broadcast.
struct BroadcastButton: View {
    let live: Bool

    var body: some View {
        ZStack {
            ActionLabel(title: live ? "หยุดส่ง" : "เริ่มส่งเสียง", stop: live)
                .allowsHitTesting(false)
            BroadcastPicker()
        }
    }
}

struct BroadcastPicker: UIViewRepresentable {
    func makeUIView(context: Context) -> RPSystemBroadcastPickerView {
        let v = RPSystemBroadcastPickerView(frame: .zero)
        v.preferredExtension = BroadcastPicker.extensionID
        v.showsMicrophoneButton = false
        v.backgroundColor = .clear
        hideGlyph(v)
        return v
    }

    func updateUIView(_ v: RPSystemBroadcastPickerView, context: Context) {
        hideGlyph(v)
        // The picker's button must cover the whole area to catch every tap.
        for case let b as UIButton in v.subviews {
            b.frame = v.bounds
            b.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        }
    }

    private func hideGlyph(_ v: UIView) {
        for s in v.subviews {
            if let b = s as? UIButton {
                b.setImage(nil, for: .normal)
                b.setImage(nil, for: .highlighted)
                b.imageView?.tintColor = .clear
                b.tintColor = .clear
            }
            hideGlyph(s)
        }
    }

    /// Sideloading tools may change bundle IDs, so read the extension's
    /// real ID from inside the app bundle instead of hard-coding it.
    static let extensionID: String? = {
        guard let plugins = Bundle.main.builtInPlugInsURL,
              let items = try? FileManager.default.contentsOfDirectory(at: plugins, includingPropertiesForKeys: nil)
        else { return nil }
        for url in items where url.pathExtension == "appex" {
            if let b = Bundle(url: url),
               let ext = b.object(forInfoDictionaryKey: "NSExtension") as? [String: Any],
               ext["NSExtensionPointIdentifier"] as? String == "com.apple.broadcast-services-upload" {
                return b.bundleIdentifier
            }
        }
        return nil
    }()
}
