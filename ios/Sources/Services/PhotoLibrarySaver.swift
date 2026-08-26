import Foundation
import Photos

enum PhotoLibrarySaveError: LocalizedError {
    case permissionDenied
    case incompatibleVideo
    case saveFailed(String)

    var errorDescription: String? {
        switch self {
        case .permissionDenied:
            return "没有照片写入权限。请到 iPhone“设置”→“YtbDown”→“照片”中允许添加照片。"
        case .incompatibleVideo:
            return "这个视频格式不能保存到系统相册。请在设置中改选“YtbDown 文件夹”后重新下载。"
        case .saveFailed(let message):
            return "保存到系统相册失败：\(message)"
        }
    }
}

enum PhotoLibrarySaver {
    static func saveVideo(at fileURL: URL) async throws {
        let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
        guard status == .authorized || status == .limited else {
            throw PhotoLibrarySaveError.permissionDenied
        }

        try await withCheckedThrowingContinuation { continuation in
            PHPhotoLibrary.shared().performChanges {
                PHAssetCreationRequest.forAsset().addResource(
                    with: .video,
                    fileURL: fileURL,
                    options: nil
                )
            } completionHandler: { success, error in
                if success {
                    continuation.resume()
                } else if let error {
                    continuation.resume(throwing: PhotoLibrarySaveError.saveFailed(error.localizedDescription))
                } else {
                    continuation.resume(throwing: PhotoLibrarySaveError.incompatibleVideo)
                }
            }
        }
    }
}
