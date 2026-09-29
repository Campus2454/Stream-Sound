import Foundation
import UIKit

/// Updates come from GitHub Releases like on every other platform. iOS
/// can't install an app by itself, so the app only tells you a newer build
/// is out and hands over to SideStore (or AltStore), which re-signs and
/// installs it from the Stream Sound source CI publishes with each release.
enum Updater {
    /// Where CI publishes releases (the public repo itself). More than one
    /// repo can be listed; the newest build found wins.
    static let repos = ["Campus2454/Stream-Sound"]
    static let asset = "StreamSound.ipa"
    static let sourceAsset = "StreamSound-source.json"

    /// SideStore reads this without a GitHub login because the repo is public.
    static var sourceURL: String { "https://github.com/\(repos[0])/releases/latest/download/\(sourceAsset)" }
    static var releasesPage: URL { URL(string: "https://github.com/\(repos[0])/releases/latest")! }

    /// "0.2" (official) or "0.2.3" (beta); tags are the same with a "v".
    struct Version: Comparable, CustomStringConvertible {
        let x: Int, y: Int, z: Int
        var beta: Bool { z > 0 }
        var description: String { beta ? "\(x).\(y).\(z)" : "\(x).\(y)" }

        init?(_ s: String) {
            let p = (s.hasPrefix("v") ? String(s.dropFirst()) : s).split(separator: ".").map { Int($0) }
            guard (2...3).contains(p.count), p.allSatisfy({ $0 != nil }) else { return nil }
            x = p[0]!; y = p[1]!; z = p.count > 2 ? p[2]! : 0
        }

        static func < (a: Version, b: Version) -> Bool { (a.x, a.y, a.z) < (b.x, b.y, b.z) }
    }

    /// This install's version (CFBundleShortVersionString, set by CI).
    static var current: Version? {
        Version(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")
    }

    static var currentText: String {
        guard let v = current, v > Version("0.1.0")! else { return "ทดสอบ (ไม่ใช่รุ่นที่เผยแพร่)" }
        return v.beta ? "\(v) (เบต้า)" : "\(v)"
    }

    struct Release: Equatable {
        let version: String
        let beta: Bool
        let page: URL
    }

    enum Check: Equatable {
        case upToDate
        case noRelease
        case newer(Release)
    }

    /// Newest release with an iPhone build across `repos`; betas only when `beta`.
    static func check(beta: Bool) async throws -> Check {
        var best: (Version, Release)?
        var any = false
        var lastError: Error?
        for repo in repos {
            do {
                for r in try await releases(repo: repo, beta: beta) {
                    any = true
                    if best == nil || r.0 > best!.0 { best = r }
                }
            } catch {
                lastError = error
            }
        }
        if !any, let e = lastError { throw e }
        guard let b = best else { return .noRelease }
        if let cur = current, b.0 <= cur { return .upToDate }
        return .newer(b.1)
    }

    private static func releases(repo: String, beta: Bool) async throws -> [(Version, Release)] {
        var req = URLRequest(url: URL(string: "https://api.github.com/repos/\(repo)/releases?per_page=30")!)
        req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        req.setValue("StreamSound-updater", forHTTPHeaderField: "User-Agent")
        req.timeoutInterval = 15
        let (data, resp) = try await URLSession.shared.data(for: req)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        if code == 404 { return [] }
        guard code == 200 else { throw URLError(.badServerResponse) }
        struct Rel: Decodable {
            struct Asset: Decodable { let name: String }
            let tag_name: String
            let draft: Bool
            let html_url: String
            let assets: [Asset]
        }
        return try JSONDecoder().decode([Rel].self, from: data).compactMap { r in
            // The iPhone build is attached a few minutes after the release appears.
            guard !r.draft, r.assets.contains(where: { $0.name == asset }),
                  let v = Version(r.tag_name), beta || !v.beta,
                  let page = URL(string: r.html_url) else { return nil }
            return (v, Release(version: v.description, beta: v.beta, page: page))
        }
    }

    /// Open SideStore (or AltStore) so it can install the update (betas:
    /// the release page, since the SideStore source follows official releases); with
    /// `addSource`, first ask it to add the Stream Sound source. Falls back
    /// to the release page in Safari. Calls back with what happened.
    @MainActor
    static func openInstaller(addSource: Bool, beta: Release? = nil, done: @escaping (String) -> Void) {
        if let b = beta {
            UIApplication.shared.open(b.page)
            done("กด StreamSound.ipa เพื่อดาวน์โหลด แล้วแชร์ไปที่ SideStore เพื่อติดตั้ง")
            return
        }
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
