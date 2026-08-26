import AVFoundation
import Foundation

struct AVFoundationMerger {
    enum MergeError: LocalizedError {
        case missingTrack
        case cannotExport
        case exportFailed(String)

        var errorDescription: String? {
            switch self {
            case .missingTrack: return "下载文件中缺少视频或音频轨道。"
            case .cannotExport: return "iPhone 无法合并这种视频编码。"
            case .exportFailed(let detail): return "合并失败：\(detail)"
            }
        }
    }

    func merge(video videoURL: URL, audio audioURL: URL, output: URL) async throws {
        let videoAsset = AVURLAsset(url: videoURL)
        let audioAsset = AVURLAsset(url: audioURL)
        let composition = AVMutableComposition()

        guard let sourceVideo = try await videoAsset.loadTracks(withMediaType: .video).first,
              let sourceAudio = try await audioAsset.loadTracks(withMediaType: .audio).first,
              let targetVideo = composition.addMutableTrack(
                withMediaType: .video,
                preferredTrackID: kCMPersistentTrackID_Invalid
              ),
              let targetAudio = composition.addMutableTrack(
                withMediaType: .audio,
                preferredTrackID: kCMPersistentTrackID_Invalid
              ) else {
            throw MergeError.missingTrack
        }

        let videoDuration = try await videoAsset.load(.duration)
        let audioDuration = try await audioAsset.load(.duration)
        try targetVideo.insertTimeRange(
            CMTimeRange(start: .zero, duration: videoDuration),
            of: sourceVideo,
            at: .zero
        )
        try targetAudio.insertTimeRange(
            CMTimeRange(start: .zero, duration: audioDuration),
            of: sourceAudio,
            at: .zero
        )
        targetVideo.preferredTransform = try await sourceVideo.load(.preferredTransform)

        guard let exporter = AVAssetExportSession(
            asset: composition,
            presetName: AVAssetExportPresetPassthrough
        ) else {
            throw MergeError.cannotExport
        }
        exporter.outputURL = output
        exporter.outputFileType = .mp4
        exporter.shouldOptimizeForNetworkUse = true

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            exporter.exportAsynchronously {
                switch exporter.status {
                case .completed:
                    continuation.resume()
                case .cancelled:
                    continuation.resume(throwing: CancellationError())
                default:
                    continuation.resume(throwing: MergeError.exportFailed(
                        exporter.error?.localizedDescription ?? "未知原因"
                    ))
                }
            }
        }
    }
}
