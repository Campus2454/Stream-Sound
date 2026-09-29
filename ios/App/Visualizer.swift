import SwiftUI

/// Frequency bars or the scrolling 2-second waveform (docs/DESIGN.md,
/// "Visualizer"). Tap to switch style. Only animates while `active`.
struct Visualizer: View {
    @EnvironmentObject var model: AppModel
    let send: Bool
    let active: Bool
    var height: CGFloat = 110
    @StateObject private var anim = VizAnimator()

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 60, paused: !active)) { tl in
            Canvas { ctx, size in
                let values = anim.frame(model: model, send: send, active: active, width: size.width, date: tl.date)
                draw(values, style: model.vizStyle, in: &ctx, size: size)
            }
        }
        .frame(height: height)
        .contentShape(Rectangle())
        .onTapGesture {
            model.vizStyle = model.vizStyle == .bars ? .wave : .bars
        }
        .accessibilityLabel(model.vizStyle == .bars ? "กราฟความถี่" : "คลื่นเสียง")
    }

    private func draw(_ v: [Float], style: VizStyle, in ctx: inout GraphicsContext, size: CGSize) {
        guard !v.isEmpty else { return }
        let mid = size.height / 2
        let slot = size.width / CGFloat(v.count)
        let barW = style == .bars ? max(2, slot * 0.62) : max(1.5, slot * 0.55)
        var upper = Path()
        var lower = Path()
        var idle = Path()
        for (i, raw) in v.enumerated() {
            let x = CGFloat(i) * slot + (slot - barW) / 2
            let h = CGFloat(raw) * (mid - 2)
            if h < 1 {
                idle.addRoundedRect(in: CGRect(x: x, y: mid - 1, width: barW, height: 2), cornerSize: CGSize(width: 1, height: 1))
                continue
            }
            let r = min(barW / 2, 3)
            upper.addRoundedRect(in: CGRect(x: x, y: mid - h, width: barW, height: h), cornerSize: CGSize(width: r, height: r))
            lower.addRoundedRect(in: CGRect(x: x, y: mid, width: barW, height: h), cornerSize: CGSize(width: r, height: r))
        }
        let grad = Gradient(colors: [Theme.redHi, Theme.red])
        ctx.fill(upper, with: .linearGradient(grad, startPoint: CGPoint(x: 0, y: 0), endPoint: CGPoint(x: 0, y: mid)))
        ctx.fill(lower, with: .linearGradient(Gradient(colors: [Theme.red.opacity(0.3), Theme.redHi.opacity(0.3)]),
                                              startPoint: CGPoint(x: 0, y: mid), endPoint: CGPoint(x: 0, y: size.height)))
        ctx.fill(idle, with: .color(Theme.text3))
    }
}

/// Keeps bar heights between frames: bars jump up at once and fall at
/// ~2.5 heights per second.
@MainActor
final class VizAnimator: ObservableObject {
    private var heights: [Float] = []
    private var buf: [Float] = []
    private var last = Date()

    func frame(model: AppModel, send: Bool, active: Bool, width: CGFloat, date: Date) -> [Float] {
        let style = model.vizStyle
        let n = style == .bars ? 32 : max(24, min(120, Int(width / 4)))
        if heights.count != n {
            heights = [Float](repeating: 0, count: n)
            buf = [Float](repeating: 0, count: n)
        }
        let dt = Float(min(0.1, max(0, date.timeIntervalSince(last))))
        last = date
        guard active else {
            heights = [Float](repeating: 0, count: n)
            return heights
        }
        let ok = model.scope(send: send, kind: style, into: &buf)
        if style == .wave {
            heights = ok ? buf : [Float](repeating: 0, count: n)
            return heights
        }
        for i in 0..<n {
            let target = ok ? buf[i] : 0
            heights[i] = max(target, heights[i] - 2.5 * dt)
        }
        return heights
    }
}

/// The volume tube: speaker (mute) + pill with a knob + percentage.
/// The live level fills the tube up to the knob.
struct VolumeTube: View {
    @EnvironmentObject var model: AppModel
    private let maxVol = 1.5

    var body: some View {
        HStack(spacing: 12) {
            Button {
                model.muted.toggle()
            } label: {
                Image(systemName: model.muted || model.volume == 0 ? "speaker.slash.fill" : model.volume < 0.6 ? "speaker.wave.1.fill" : "speaker.wave.3.fill")
                    .font(.system(size: 17, weight: .semibold))
                    .foregroundColor(model.muted ? Theme.text3 : Theme.text)
                    .frame(width: 36, height: 36)
                    .background(Theme.surface2, in: Circle())
            }
            .buttonStyle(PressStyle())
            .accessibilityLabel(model.muted ? "เปิดเสียง" : "ปิดเสียง")

            GeometryReader { geo in
                let w = geo.size.width
                let knobD: CGFloat = 22
                let track = w - knobD
                let pos = CGFloat(model.volume / maxVol)
                let knobX = knobD / 2 + track * pos
                let fill = model.muted ? 0 : CGFloat(model.receiveLevel) * knobX
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.surface2)
                    Capsule()
                        .fill(LinearGradient(colors: [Theme.red, Theme.redHi], startPoint: .leading, endPoint: .trailing))
                        .frame(width: max(0, fill))
                        .animation(.linear(duration: 0.1), value: fill)
                    // 100 % mark.
                    Rectangle().fill(Theme.text3)
                        .frame(width: 2, height: 10)
                        .offset(x: knobD / 2 + track * CGFloat(1 / maxVol) - 1)
                    Circle()
                        .fill(Color.white)
                        .overlay(Circle().stroke(Theme.red, lineWidth: 3))
                        .frame(width: knobD, height: knobD)
                        .shadow(color: .black.opacity(0.4), radius: 3, y: 1)
                        .offset(x: knobX - knobD / 2)
                }
                .frame(height: 28)
                .frame(maxHeight: .infinity)
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 0).onChanged { g in
                        var v = Double((g.location.x - knobD / 2) / max(track, 1)) * maxVol
                        v = v.clamped(0, maxVol)
                        if abs(v - 1) < 0.04 { v = 1 } // snap to 100 %
                        model.volume = v
                        if model.muted { model.muted = false }
                    }
                )
            }
            .frame(height: 36)
            .accessibilityElement()
            .accessibilityLabel("ระดับเสียง")
            .accessibilityValue("\(Int((model.volume * 100).rounded())) เปอร์เซ็นต์")
            .accessibilityAdjustableAction { dir in
                switch dir {
                case .increment: model.volume = min(maxVol, model.volume + 0.05)
                case .decrement: model.volume = max(0, model.volume - 0.05)
                @unknown default: break
                }
            }

            Text("\(Int((model.volume * 100).rounded()))%")
                .font(.system(size: 14, weight: .semibold).monospacedDigit())
                .foregroundColor(model.muted ? Theme.text3 : Theme.text)
                .frame(width: 48, alignment: .trailing)
        }
    }
}
