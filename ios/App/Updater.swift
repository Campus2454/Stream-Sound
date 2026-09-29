import Foundation
import UIKit

/// Updates come from GitHub Releases like on every other platform. iOS
/// can't install an app by itself, so the app only tells you a newer build
/// is out and hands over to SideStore (or AltStore), which re-signs and
/// installs it from the Stream Sound source CI publishes with each release.
enum Updater {
    /// Where CI publishes releases (the same place the desktop and Android apps update from).
    static let repo = "Campus2454/Audio-Streaming"
    static let asset = "StreamSound.ipa"
    static let sourceAsset = "StreamSound-source.json"

    static var sourceURL: String { "https://github.com/\(repo)/releases/latest/download/\(sourceAsset)" }
    static var releasesPage: URL { URL(string: "https://github.com/\(repo)/releases/latest")! }

    static var currentBuild: Int {
        Int(Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "") ?? 0
    }

    enum Check: Equatable {
        case upToDate
        case noRelease
        case newer(build: Int)
    }

    static func check() async throws -> Check {
        var req = URLRequest(url: URL(string: "https://api.github.com/repos/\(repo)/releases/latest")!)
        req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        req.setValue("StreamSound-updater", forHTTPHeaderField: "User-Agent")
        req.timeoutInterval = 15
        let (data, resp) = try await URLSession.shared.data(for: req)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        if code == 404 { return .noRelease }
        guard code == 200 else { throw URLError(.badServerResponse) }
        struct Rel: Decodable {
            struct Asset: Decodable { let name: String }
            let tag_name: String
            let assets: [Asset]
        }
        let rel = try JSONDecoder().decode(Rel.self, from: data)
        guard let build = Int(rel.tag_name.split(separator: ".").last ?? "") else { return .noRelease }
        // The iPhone build is attached a few minutes after the release appears.
        guard build > currentBuild, rel.assets.contains(where: { $0.name == asset }) else { return .upToDate }
        return .newer(build: build)
    }

    /// Open SideStore (or AltStore) so it can install the update; with
    /// `addSource`, first ask it to add the Stream Sound source. Falls back
    /// to the release page in Safari. Calls back with what happened.
    @MainActor
    static func openInstaller(addSource: Bool, done: @escaping (String) -> Void) {
        let enc = sourceURL.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? sourceURL
        let tries: [(String, String)] = addSource
            ? [("sidestore://source?url=\(enc)", "เปิด SideStore แล้ว กดเพิ่มแหล่ง Stream Sound"),
               ("altstore://source?url=\(enc)", "เปิด AltStore แล้ว กดเพิ่มแหล่ง Stream Sound")]
            : [("sidestore://", "เปิด SideStore แล้ว ไปที่แท็บ Updates เพื่ออัปเดต"),
               ("altstore://", "เปิด AltStore แล้ว ไปที่แท็บ Updates เพื่ออัปเดต")]
        func attempt(_ i: Int) {
            guard i < tries.count, let url = URL(string: tries[i].0) else {
                UIPasteboard.general.string = sourceURL
                UIApplication.shared.open(releasesPage)
                done("ไม่พบ SideStore จึงเปิดหน้าดาวน์โหลดแทน (คัดลอกลิงก์แหล่งอัปเดตไว้ให้แล้ว)")
                return
            }
            UIApplication.shared.open(url, options: [:]) { ok in
                if ok { done(tries[i].1) } else { attempt(i + 1) }
            }
        }
        attempt(0)
    }
}
