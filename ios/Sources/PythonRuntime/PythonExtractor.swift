import Foundation

final class PythonExtractor {
    static let shared = PythonExtractor()

    private let queue = DispatchQueue(label: "com.litotime.ytbdown.ios.python")
    private var initialized = false

    private init() {}

    func listFormats(_ url: URL) async throws -> FormatListing {
        try await perform {
            guard let result = keraunos_python_list_formats(url.absoluteString, nil) else {
                throw LocalExtractionError.runtime("本地解析器没有返回结果。")
            }
            defer { free(result) }
            return try ExtractionDecoder.decodeListing(Data(String(cString: result).utf8))
        }
    }

    func resolve(_ url: URL, option: FormatOption) async throws -> ResolvedMedia {
        try await perform {
            guard let result = keraunos_python_extract(
                url.absoluteString,
                nil,
                option.formatID,
                option.isAdaptive ? 1 : 0
            ) else {
                throw LocalExtractionError.runtime("本地解析器没有返回结果。")
            }
            defer { free(result) }
            return try ExtractionDecoder.decode(Data(String(cString: result).utf8))
        }
    }

    func downloadTrack(_ track: MediaTrack, to destination: URL) async throws {
        struct DownloadResult: Decodable {
            let ok: Bool
            let errorKind: String?
            let detail: String?

            enum CodingKeys: String, CodingKey {
                case ok, detail
                case errorKind = "error_kind"
            }
        }

        let headersData = try JSONEncoder().encode(track.httpHeaders)
        let headersJSON = String(decoding: headersData, as: UTF8.self)
        try await perform {
            guard let result = keraunos_python_download_track(
                track.url.absoluteString,
                headersJSON,
                destination.path
            ) else {
                throw LocalExtractionError.runtime("本地下载器没有返回结果。")
            }
            defer { free(result) }
            let payload = try JSONDecoder().decode(
                DownloadResult.self,
                from: Data(String(cString: result).utf8)
            )
            guard payload.ok else {
                if payload.errorKind == "download_http" {
                    throw LocalExtractionError.runtime("媒体服务器拒绝下载（\(payload.detail ?? "HTTP 错误")）。")
                }
                throw LocalExtractionError(kind: payload.errorKind, detail: payload.detail)
            }
        }
    }

    private func perform<T>(_ work: @escaping () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    try self.initializeIfNeeded()
                    continuation.resume(returning: try work())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private func initializeIfNeeded() throws {
        guard !initialized else { return }
        let candidates = [Bundle.main.resourceURL, Bundle(for: PythonExtractor.self).resourceURL]
            .compactMap { $0 }
        guard let resources = candidates.first(where: {
            FileManager.default.fileExists(atPath: $0.appendingPathComponent("app/keraunos_extract.py").path)
        }) else {
            throw LocalExtractionError.runtime("找不到 App 资源目录。")
        }
        let certificate = resources.appendingPathComponent("app/cacert.pem")
        let status = keraunos_python_init(resources.path, certificate.path)
        guard status == 0 else {
            throw LocalExtractionError.runtime("Python 初始化失败（\(status)）。")
        }
        initialized = true
    }
}
