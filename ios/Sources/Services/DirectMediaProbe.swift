import Foundation

struct DirectMediaProbe {
    private static let knownPageHosts = [
        "youtube.com", "youtu.be", "bilibili.com", "b23.tv",
        "x.com", "twitter.com", "tiktok.com", "douyin.com",
        "v.qq.com", "pinterest.com", "pin.it"
    ]

    static func validate(text: String) throws -> URL {
        let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: value), let scheme = url.scheme?.lowercased(), url.host != nil else {
            throw DownloadEngineError.invalidURL
        }
        guard scheme == "https" else {
            throw DownloadEngineError.insecureURL
        }
        let host = url.host?.lowercased() ?? ""
        if knownPageHosts.contains(where: { host == $0 || host.hasSuffix(".\($0)") }) {
            throw DownloadEngineError.unsupportedPage
        }
        return url
    }

    static func inspect(_ url: URL, session: URLSession = .shared) async throws -> DirectMediaInfo {
        var request = URLRequest(url: url)
        request.httpMethod = "HEAD"
        request.timeoutInterval = 20
        let (_, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw DownloadEngineError.invalidResponse
        }
        guard (200..<400).contains(http.statusCode) else {
            throw DownloadEngineError.serverStatus(http.statusCode)
        }

        let mimeType = http.mimeType?.lowercased()
        let disposition = http.value(forHTTPHeaderField: "Content-Disposition")?.lowercased() ?? ""
        let isMedia = mimeType?.hasPrefix("video/") == true
            || mimeType?.hasPrefix("audio/") == true
            || mimeType == "application/octet-stream"
            || disposition.contains("attachment")
        guard isMedia else {
            throw DownloadEngineError.notDirectMedia(mimeType)
        }

        let filename = response.suggestedFilename
            ?? url.lastPathComponent.nonEmpty
            ?? "download"
        let size = http.expectedContentLength > 0 ? http.expectedContentLength : nil
        return DirectMediaInfo(url: url, filename: filename, mimeType: mimeType, expectedBytes: size)
    }
}
private extension String {
    var nonEmpty: String? { isEmpty ? nil : self }
}
