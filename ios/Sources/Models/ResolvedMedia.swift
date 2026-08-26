import Foundation

struct MediaTrack: Codable, Equatable {
    let url: URL
    let httpHeaders: [String: String]
    let codec: String
    let fileExtension: String
    let chunkSize: Int?
    let approxBytes: Int64?
}

struct ResolvedMedia: Equatable {
    enum Kind: Equatable {
        case progressive(MediaTrack)
        case adaptive(video: MediaTrack, audio: MediaTrack)
    }

    let kind: Kind
    let title: String
    let suggestedFilename: String
}

struct FormatOption: Identifiable, Equatable {
    var id: String { "\(formatID)-\(height)" }
    let height: Int
    let codecLabel: String
    let approxBytes: Int64?
    let formatID: String
    let isAdaptive: Bool

    var displayLabel: String {
        var parts = ["\(height)p"]
        if !codecLabel.isEmpty { parts.append(codecLabel) }
        if let approxBytes {
            parts.append(ByteCountFormatter.string(fromByteCount: approxBytes, countStyle: .file))
        }
        return parts.joined(separator: " · ")
    }
}

enum FormatListing {
    case ready(ResolvedMedia)
    case choices([FormatOption])
}

private struct TrackPayload: Decodable {
    let url: String
    let headers: [String: String]?
    let vcodec: String?
    let acodec: String?
    let ext: String?
    let chunkSize: Int?
    let approxBytes: Int64?

    enum CodingKeys: String, CodingKey {
        case url, headers, vcodec, acodec, ext
        case chunkSize = "chunk_size"
        case approxBytes = "approx_bytes"
    }
}

private struct ExtractionPayload: Decodable {
    let ok: Bool
    let kind: String?
    let title: String?
    let filename: String?
    let media: TrackPayload?
    let video: TrackPayload?
    let audio: TrackPayload?
    let errorKind: String?
    let detail: String?

    enum CodingKeys: String, CodingKey {
        case ok, kind, title, filename, media, video, audio, detail
        case errorKind = "error_kind"
    }
}

enum ExtractionDecoder {
    static func decode(_ data: Data) throws -> ResolvedMedia {
        let payload: ExtractionPayload
        do {
            payload = try JSONDecoder().decode(ExtractionPayload.self, from: data)
        } catch {
            throw LocalExtractionError.runtime("解析器返回了无法识别的数据。")
        }
        guard payload.ok else {
            throw LocalExtractionError(kind: payload.errorKind, detail: payload.detail)
        }

        switch payload.kind {
        case "progressive":
            guard let track = track(payload.media) else {
                throw LocalExtractionError.runtime("缺少可下载的媒体地址。")
            }
            return ResolvedMedia(
                kind: .progressive(track),
                title: payload.title ?? "",
                suggestedFilename: filename(payload.filename, fallback: track.url)
            )
        case "adaptive":
            guard let video = track(payload.video), let audio = track(payload.audio) else {
                throw LocalExtractionError.runtime("缺少视频或音频轨道。")
            }
            return ResolvedMedia(
                kind: .adaptive(video: video, audio: audio),
                title: payload.title ?? "",
                suggestedFilename: filename(payload.filename, fallback: video.url)
            )
        default:
            throw LocalExtractionError.runtime("解析器返回了未知格式。")
        }
    }

    static func decodeListing(_ data: Data) throws -> FormatListing {
        struct Envelope: Decodable {
            struct Option: Decodable {
                let height: Int
                let codec: String?
                let approxBytes: Int64?
                let formatID: String
                let adaptive: Bool

                enum CodingKeys: String, CodingKey {
                    case height, codec, adaptive
                    case approxBytes = "approx_bytes"
                    case formatID = "format_id"
                }
            }

            let ok: Bool
            let kind: String?
            let options: [Option]?
            let errorKind: String?
            let detail: String?

            enum CodingKeys: String, CodingKey {
                case ok, kind, options, detail
                case errorKind = "error_kind"
            }
        }

        let envelope: Envelope
        do {
            envelope = try JSONDecoder().decode(Envelope.self, from: data)
        } catch {
            throw LocalExtractionError.runtime("解析器返回了无法识别的数据。")
        }
        guard envelope.ok else {
            throw LocalExtractionError(kind: envelope.errorKind, detail: envelope.detail)
        }
        guard envelope.kind == "choices" else { return .ready(try decode(data)) }
        return .choices((envelope.options ?? []).map {
            FormatOption(
                height: $0.height,
                codecLabel: $0.codec ?? "",
                approxBytes: $0.approxBytes,
                formatID: $0.formatID,
                isAdaptive: $0.adaptive
            )
        })
    }

    private static func track(_ value: TrackPayload?) -> MediaTrack? {
        guard let value, let url = URL(string: value.url) else { return nil }
        return MediaTrack(
            url: url,
            httpHeaders: value.headers ?? [:],
            codec: value.vcodec ?? value.acodec ?? "",
            fileExtension: value.ext ?? url.pathExtension,
            chunkSize: value.chunkSize,
            approxBytes: value.approxBytes
        )
    }

    private static func filename(_ value: String?, fallback: URL) -> String {
        guard let value, !value.isEmpty else { return fallback.lastPathComponent }
        return value
    }
}

enum LocalExtractionError: LocalizedError, Equatable {
    case unsupported
    case needsMergeSupport
    case requiresLogin
    case network
    case runtime(String)
    case timedOut
    case unavailable
    case rateLimited
    case restrictedOrEmpty

    init(kind: String?, detail: String?) {
        switch kind {
        case "unsupported": self = .unsupported
        case "needs_ffmpeg": self = .needsMergeSupport
        case "requires_auth": self = .requiresLogin
        case "extract_network", "download_network", "network": self = .network
        case "timeout": self = .timedOut
        case "unavailable": self = .unavailable
        case "rate_limited": self = .rateLimited
        case "restricted_or_empty": self = .restrictedOrEmpty
        default: self = .runtime((detail?.isEmpty == false ? detail : nil) ?? "本地解析器运行失败。")
        }
    }

    var errorDescription: String? {
        switch self {
        case .unsupported: return "暂不支持这个网站或链接。"
        case .needsMergeSupport: return "这个视频只有当前版本不能合并的编码格式。"
        case .requiresLogin: return "这个内容需要登录；首版暂不导入账号 Cookie。"
        case .network: return "读取页面失败，请检查网络后重试。"
        case .runtime(let detail): return "本地解析失败：\(detail)"
        case .timedOut: return "页面解析超时，请稍后重试。"
        case .unavailable: return "视频可能已删除、设为私密或有地区限制。"
        case .rateLimited: return "网站正在限制请求，请稍后再试。"
        case .restrictedOrEmpty: return "没有找到可下载视频；敏感内容可能需要登录。"
        }
    }
}
