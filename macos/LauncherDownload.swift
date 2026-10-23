import Foundation

/// URLSession performs TLS validation. This delegate additionally limits every
/// redirect to the exact publisher/CDN hosts bundled with the catalog entry.
final class LauncherDownload: NSObject, URLSessionDownloadDelegate, @unchecked Sendable {
    static let maximumSize: Int64 = 512 * 1024 * 1024
    let hosts: [String]
    let progress: @Sendable (Int64, Int64) -> Void
    init(hosts: [String], progress: @escaping @Sendable (Int64, Int64) -> Void) {
        self.hosts = hosts; self.progress = progress
    }
    static func allows(_ url: URL, hosts: [String]) -> Bool {
        url.scheme?.lowercased() == "https" && url.user == nil && url.password == nil &&
        (url.port == nil || url.port == 443) && hosts.contains(url.host?.lowercased() ?? "")
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping @Sendable (URLRequest?) -> Void) {
        completionHandler(request.url.map { Self.allows($0, hosts: hosts) } == true ? request : nil)
    }
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64,
                    totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        if totalBytesWritten > Self.maximumSize || totalBytesExpectedToWrite > Self.maximumSize { downloadTask.cancel(); return }
        progress(totalBytesWritten, totalBytesExpectedToWrite)
    }
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {}
}
