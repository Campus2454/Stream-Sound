import SwiftUI

/// Colours and sizes from docs/DESIGN.md, shared by every Stream Sound app.
enum Theme {
    static let bg = Color(hex: 0x0A0A0C)
    static let surface = Color(hex: 0x141417)
    static let surface2 = Color(hex: 0x1D1D22)
    static let outline = Color(hex: 0x2A2A31)
    static let red = Color(hex: 0xE62639)
    static let redHi = Color(hex: 0xFF5A6A)
    static let redDeep = Color(hex: 0x8C1320)
    static let text = Color(hex: 0xF4F4F6)
    static let text2 = Color(hex: 0xA3A3AD)
    static let text3 = Color(hex: 0x6E6E78)
    static let green = Color(hex: 0x3DDC84)
    static let amber = Color(hex: 0xFFB020)
    static let error = Color(hex: 0xFF6B6B)

    static let cardRadius: CGFloat = 16
    static let buttonRadius: CGFloat = 14
    static let page: CGFloat = 16
    static let gap: CGFloat = 12

    static let redGradient = LinearGradient(colors: [redHi, red], startPoint: .top, endPoint: .bottom)
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: opacity)
    }
}
