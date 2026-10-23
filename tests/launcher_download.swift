import Foundation

@main struct DownloadCheck {
    static func main() async throws {
        let hosts = ["itch.io", "broth.itch.zone", "itchio-mirror.cb031a832f44726753d6267436f3b414.r2.cloudflarestorage.com"]
        for rejected in ["http://itch.io/app", "https://itch.io.evil.example/app", "https://user:pass@itch.io/app", "https://itch.io:444/app"] {
            precondition(!LauncherDownload.allows(URL(string:rejected)!,hosts:hosts))
        }
        guard CommandLine.arguments.contains("--network") else { print("PASS URL restrictions"); return }
        let session = URLSession(configuration:.ephemeral)
        defer { session.invalidateAndCancel() }
        let delegate = LauncherDownload(hosts:hosts) { _,_ in }
        let (file,response) = try await session.download(for:URLRequest(url:URL(string:"https://itch.io/app/download?platform=windows")!),delegate:delegate)
        defer { try? FileManager.default.removeItem(at:file) }
        precondition((response as? HTTPURLResponse)?.statusCode == 200)
        precondition(LauncherDownload.allows(response.url!,hosts:hosts))
        let handle = try FileHandle(forReadingFrom:file); defer { try? handle.close() }
        let magic = try handle.read(upToCount:2)
        precondition(magic == Data([0x4d,0x5a]))
        print("PASS official itch.io download, allowed HTTPS redirects and Windows installer header")
    }
}
