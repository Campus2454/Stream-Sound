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

    /// Newest release across `repos`; betas only when `beta`. Uses GitHub's
    /// web pages, not its API: the API allows only 60 anonymous calls an
    /// hour per home IP, too few for every device checking every 5 minutes.
    static func check(beta: Bool) async throws -> Check {
        var best: (Version, String, String)?  // version, tag, repo
        var lastError: Error?
        for repo in repos {
            do {
                if let t = try await newestTag(repo: repo, beta: beta), best == nil || t.0 > best!.0 {
                    best = (t.0, t.1, repo)
                }
            } catch {
                lastError = error
            }
        }
        guard let b = best else {
            if let e = lastError { throw e }
            return .noRelease
        }
        let (v, tag, repo) = b
        if let cur = current, v <= cur { return .upToDate }
        // The iPhone build is attached a few minutes after the release
        // appears; until then say nothing and look again next time.
        guard try await exists("https://github.com/\(repo)/releases/download/\(tag)/\(asset)") else { return .upToDate }
        let page = URL(string: "https://github.com/\(repo)/releases/tag/\(tag)")!
        return .newer(Release(version: v.description, beta: v.beta, page: page))
    }

    private static func request(_ url: String, method: String = "GET") -> URLRequest {
        var req = URLRequest(url: URL(string: url)!)
        req.httpMethod = method
        req.setValue("StreamSound-updater", forHTTPHeaderField: "User-Agent")
        req.cachePolicy = .reloadIgnoringLocalCacheData
        req.timeoutInterval = 15
        return req
    }

    private static func newestTag(repo: String, beta: Bool) async throws -> (Version, String)? {
        if beta {
            // The last 10 releases, betas (pre-releases) included.
            let (data, resp) = try await URLSession.shared.data(for: request("https://github.com/\(repo)/releases.atom"))
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
            if code == 404 { return nil }
            guard code == 200, let text = String(data: data, encoding: .utf8) else { throw URLError(.badServerResponse) }
            let re = try NSRegularExpression(pattern: "/releases/tag/(v[0-9.]+)")
            let tags = re.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap { m -> (Version, String)? in
                guard let r = Range(m.range(at: 1), in: text) else { return nil }
                let tag = String(text[r])
                return Version(tag).map { ($0, tag) }
            }
            return tags.max { $0.0 < $1.0 }
        }
        // Official only: /releases/latest redirects to /releases/tag/vX.Y.
        let (_, resp) = try await noRedirect.data(for: request("https://github.com/\(repo)/releases/latest"))
        guard let http = resp as? HTTPURLResponse else { throw URLError(.badServerResponse) }
        if http.statusCode == 404 { return nil }
        guard (300..<400).contains(http.statusCode),
              let loc = http.value(forHTTPHeaderField: "Location"),
              let r = loc.range(of: "/releases/tag/") else { return nil }
        let tag = String(loc[r.upperBound...]).components(separatedBy: CharacterSet(charactersIn: "/?#")).first ?? ""
        return Version(tag).map { ($0, tag) }
    }

    private static func exists(_ url: String) async throws -> Bool {
        let (_, resp) = try await URLSession.shared.data(for: request(url, method: "HEAD"))
        return (resp as? HTTPURLResponse)?.statusCode == 200
    }

    private final class StopRedirects: NSObject, URLSessionTaskDelegate {
        func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                        newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
            completionHandler(nil)
        }
    }

    private static let noRedirect = URLSession(configuration: .ephemeral, delegate: StopRedirects(), delegateQueue: nil)

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
