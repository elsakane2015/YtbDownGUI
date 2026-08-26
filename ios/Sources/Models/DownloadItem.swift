import Foundation

enum SaveDestination: String, Codable, CaseIterable, Identifiable {
    case photoLibrary
    case appFolder

    static let storageKey = "ytbdown.ios.save-destination"

    var id: String { rawValue }

    var title: String {
        switch self {
        case .photoLibrary: return "系统相册"
        case .appFolder: return "YtbDown 文件夹"
        }
    }

    var detail: String {
        switch self {
        case .photoLibrary: return "保存在“照片”App 中"
        case .appFolder: return "保存在“文件”App → 我的 iPhone → YtbDown"
        }
    }
}

struct DownloadItem: Codable, Identifiable, Equatable {
    enum State: String, Codable {
        case queued
        case running
        case merging
        case saving
        case completed
        case failed
        case canceled

        var title: String {
            switch self {
            case .queued: return "等待中"
            case .running: return "下载中"
            case .merging: return "正在合并"
            case .saving: return "正在保存"
            case .completed: return "已完成"
            case .failed: return "失败"
            case .canceled: return "已取消"
            }
        }
    }

    let id: UUID
    let sourceURL: URL
    var title: String
    var state: State
    var progress: Double
    var receivedBytes: Int64
    var totalBytes: Int64?
    var localFilename: String?
    var savedDestination: SaveDestination?
    var errorMessage: String?
    let createdAt: Date

    init(sourceURL: URL, title: String, totalBytes: Int64?) {
        self.id = UUID()
        self.sourceURL = sourceURL
        self.title = title
        self.state = .queued
        self.progress = 0
        self.receivedBytes = 0
        self.totalBytes = totalBytes
        self.localFilename = nil
        self.savedDestination = nil
        self.errorMessage = nil
        self.createdAt = Date()
    }
}

struct DirectMediaInfo: Equatable {
    let url: URL
    let filename: String
    let mimeType: String?
    let expectedBytes: Int64?
}

enum DownloadEngineError: LocalizedError, Equatable {
    case invalidURL
    case insecureURL
    case unsupportedPage
    case notDirectMedia(String?)
    case invalidResponse
    case serverStatus(Int)

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "请输入完整的下载链接。"
        case .insecureURL:
            return "iOS 版只接受 HTTPS 下载链接。"
        case .unsupportedPage:
            return "这是视频网站页面，请使用首页的本地页面解析功能。"
        case .notDirectMedia(let type):
            if let type, !type.isEmpty {
                return "服务器返回了 \(type)，它看起来不是可下载的媒体文件。"
            }
            return "这个地址看起来不是可下载的媒体直链。"
        case .invalidResponse:
            return "服务器返回了无法识别的响应。"
        case .serverStatus(let code):
            return "服务器返回错误状态 \(code)。"
        }
    }
}
