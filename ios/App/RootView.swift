import SwiftUI

@main
struct StreamSoundApp: App {
    @StateObject private var model = AppModel()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(model)
                .preferredColorScheme(.dark)
                .onChange(of: phase) { p in model.setActive(p == .active) }
        }
    }
}

struct RootView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        VStack(spacing: 0) {
            Header()
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.gap) {
                    if let r = model.newer {
                        UpdateBanner(release: r)
                    }
                    switch model.tab {
                    case .send: SendView()
                    case .receive: ReceiveView()
                    case .settings: SettingsView()
                    }
                }
                .padding(.horizontal, Theme.page)
                .padding(.bottom, 24)
            }
            BottomBar()
        }
        .background(Theme.bg.ignoresSafeArea())
        .overlay(alignment: .bottom) {
            if let t = model.toast {
                Text(t)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(Theme.text)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(Theme.surface2, in: Capsule())
                    .overlay(Capsule().stroke(Theme.outline, lineWidth: 1))
                    .padding(.horizontal, 24)
                    .padding(.bottom, 84)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .onTapGesture { model.toast = nil }
            }
        }
        .animation(.spring(response: 0.3, dampingFraction: 0.85), value: model.toast)
    }
}

struct LogoMark: View {
    var size: CGFloat = 30
    var body: some View {
        RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
            .fill(Theme.redGradient)
            .frame(width: size, height: size)
            .overlay(
                HStack(alignment: .center, spacing: size * 0.07) {
                    ForEach(Array(([210, 420, 600, 360, 240] as [CGFloat]).enumerated()), id: \.offset) { h in
                        Capsule().fill(Color.white).frame(width: size * 0.09, height: size * h.element / 1024)
                    }
                }
            )
    }
}

struct Header: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        HStack(spacing: 10) {
            LogoMark()
            Text("Stream Sound").font(.system(size: 19, weight: .bold)).foregroundColor(Theme.text)
            Spacer(minLength: 8)
            Button {
                if let ip = model.state.ips.first { model.copy(ip) }
            } label: {
                HStack(spacing: 6) {
                    Circle().fill(model.state.ips.isEmpty ? Theme.text3 : Theme.green).frame(width: 7, height: 7)
                    Text(chipText)
                        .font(.system(size: 12, weight: .medium).monospacedDigit())
                        .foregroundColor(Theme.text2)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Theme.surface, in: Capsule())
                .overlay(Capsule().stroke(Theme.outline, lineWidth: 1))
            }
            .buttonStyle(PressStyle())
        }
        .padding(.horizontal, Theme.page)
        .padding(.vertical, 10)
    }

    private var chipText: String {
        let ip = model.state.ips.first ?? "ไม่ได้ต่อ Wi‑Fi"
        return "\(model.state.name) · \(ip)"
    }
}

struct UpdateBanner: View {
    @EnvironmentObject var model: AppModel
    let release: Updater.Release

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("มีเวอร์ชันใหม่ \(release.version)\(release.beta ? " (เบต้า)" : "")").font(.system(size: 15, weight: .bold)).foregroundColor(.white)
                Text(release.beta ? "ดาวน์โหลดแล้วติดตั้งผ่าน SideStore" : "ติดตั้งผ่าน SideStore").font(.system(size: 12)).foregroundColor(.white.opacity(0.8))
            }
            Spacer()
            Button("อัปเดตเลย") { model.installUpdate(addSource: false) }
                .font(.system(size: 14, weight: .bold))
                .foregroundColor(Theme.red)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(Color.white, in: Capsule())
        }
        .padding(14)
        .background(LinearGradient(colors: [Theme.redHi, Theme.red, Theme.redDeep], startPoint: .topLeading, endPoint: .bottomTrailing),
                    in: RoundedRectangle(cornerRadius: Theme.cardRadius, style: .continuous))
    }
}

struct BottomBar: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        HStack(spacing: 0) {
            item(.send, "ส่งเสียง", "dot.radiowaves.left.and.right", dot: model.sending ? Theme.red : nil)
            item(.receive, "รับเสียง", "hifispeaker.fill", dot: model.state.streams.isEmpty ? nil : Theme.green)
            item(.settings, "ตั้งค่า", "gearshape.fill", dot: nil)
        }
        .padding(.top, 8)
        .padding(.bottom, 2)
        .background(Theme.surface.ignoresSafeArea(edges: .bottom))
        .overlay(alignment: .top) { Rectangle().fill(Theme.outline).frame(height: 1) }
    }

    private func item(_ tab: AppTab, _ title: String, _ icon: String, dot: Color?) -> some View {
        let on = model.tab == tab
        return Button {
            model.tab = tab
        } label: {
            VStack(spacing: 4) {
                ZStack(alignment: .topTrailing) {
                    Image(systemName: icon).font(.system(size: 20, weight: .semibold))
                        .frame(width: 44, height: 26)
                    if let c = dot {
                        Circle().fill(c).frame(width: 8, height: 8)
                            .overlay(Circle().stroke(Theme.surface, lineWidth: 2))
                            .offset(x: -4, y: -1)
                    }
                }
                Text(title).font(.system(size: 11, weight: on ? .semibold : .regular))
            }
            .foregroundColor(on ? Theme.red : Theme.text3)
            .frame(maxWidth: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
