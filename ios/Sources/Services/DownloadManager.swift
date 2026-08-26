import Foundation
import UIKit

private enum MediaTransferError: LocalizedError {
    case httpStatus(Int)

    var errorDescription: String? {
        switch self {
        case .httpStatus(let status):
            return "媒体服务器拒绝下载（HTTP \(status)）。请重新解析后再试。"
        }
    }
}

final class DownloadManager: NSObject, ObservableObject {
    static let backgroundSessionIdentifier = "com.litotime.ytbdowngui.ios.downloads"

    @Published private(set) var items: [DownloadItem] = []

    private struct DownloadJob: Codable {
        let sourceURL: URL
        let suggestedFilename: String
        let tracks: [MediaTrack]
        let needsMerge: Bool
        var completed: [Bool]
        let saveDestination: SaveDestination?
    }

    private struct TaskContext {
        let itemID: UUID
        let trackIndex: Int
        let startingOffset: Int64
    }

    private let itemStorageKey = "ytbdown.ios.download-items.v2"
    private let jobStorageKey = "ytbdown.ios.download-jobs.v1"
    private var jobs: [String: DownloadJob] = [:]
    private lazy var session: URLSession = {
        let configuration = URLSessionConfiguration.background(
            withIdentifier: Self.backgroundSessionIdentifier
        )
        configuration.waitsForConnectivity = true
        configuration.allowsCellularAccess = true
        configuration.sessionSendsLaunchEvents = true
        return URLSession(configuration: configuration, delegate: self, delegateQueue: nil)
    }()

    override init() {
        super.init()
        restore()
        reconnectBackgroundTasks()
    }

    @MainActor
    func enqueue(_ media: ResolvedMedia, sourceURL: URL) {
        let tracks: [MediaTrack]
        let needsMerge: Bool
        switch media.kind {
        case .progressive(let track):
            tracks = [track]
            needsMerge = false
        case .adaptive(let video, let audio):
            tracks = [video, audio]
            needsMerge = true
        }

        var item = DownloadItem(
            sourceURL: sourceURL,
            title: media.title.isEmpty ? media.suggestedFilename : media.title,
            totalBytes: estimatedTotal(tracks)
        )
        item.state = .running
        items.insert(item, at: 0)
        jobs[item.id.uuidString] = DownloadJob(
            sourceURL: sourceURL,
            suggestedFilename: media.suggestedFilename,
            tracks: tracks,
            needsMerge: needsMerge,
            completed: Array(repeating: false, count: tracks.count),
            saveDestination: selectedSaveDestination
        )
        persist()
        startTrack(itemID: item.id, index: 0)
    }

    @MainActor
    func enqueue(_ media: DirectMediaInfo) {
        let track = MediaTrack(
            url: media.url,
            httpHeaders: [:],
            codec: "",
            fileExtension: media.url.pathExtension,
            chunkSize: nil,
            approxBytes: media.expectedBytes
        )
        enqueue(
            ResolvedMedia(
                kind: .progressive(track),
                title: media.filename,
                suggestedFilename: media.filename
            ),
            sourceURL: media.url
        )
    }

    @MainActor
    func cancel(_ item: DownloadItem) {
        session.getAllTasks { tasks in
            for task in tasks where task.taskDescription?.hasPrefix(item.id.uuidString) == true {
                task.cancel()
            }
        }
        update(item.id) {
            $0.state = .canceled
            $0.errorMessage = nil
        }
    }

    @MainActor
    func removeRecord(_ item: DownloadItem) {
        items.removeAll { $0.id == item.id }
        removeParts(itemID: item.id)
        jobs[item.id.uuidString] = nil
        persist()
    }

    func localFileURL(for item: DownloadItem) -> URL? {
        guard let filename = item.localFilename else { return nil }
        let url = Self.documentsDirectory.appendingPathComponent(filename)
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }

    private static var documentsDirectory: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    private static var partsDirectory: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("DownloadParts", isDirectory: true)
    }

    private static var photoStagingDirectory: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("PhotoStaging", isDirectory: true)
    }

    private var selectedSaveDestination: SaveDestination {
        let rawValue = UserDefaults.standard.string(forKey: SaveDestination.storageKey)
        return rawValue.flatMap(SaveDestination.init(rawValue:)) ?? .appFolder
    }

    private func partURL(itemID: UUID, trackIndex: Int, track: MediaTrack) -> URL {
        let fallback = trackIndex == 0 ? "mp4" : "m4a"
        let ext = track.fileExtension.isEmpty ? fallback : track.fileExtension
        return Self.partsDirectory.appendingPathComponent("\(itemID.uuidString)-\(trackIndex).\(ext)")
    }

    private func startTrack(itemID: UUID, index: Int) {
        guard let job = jobs[itemID.uuidString], job.tracks.indices.contains(index) else { return }
        let track = job.tracks[index]
        let part = partURL(itemID: itemID, trackIndex: index, track: track)
        try? FileManager.default.createDirectory(at: Self.partsDirectory, withIntermediateDirectories: true)
        if isGoogleVideo(track) {
            startPythonTrack(itemID: itemID, index: index, track: track, destination: part)
            return
        }
        let offset = (try? part.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? 0

        var request = URLRequest(url: track.url)
        request.timeoutInterval = 60
        for (field, value) in track.httpHeaders {
            request.setValue(value, forHTTPHeaderField: field)
        }
        if let chunkSize = effectiveChunkSize(for: track) {
            request.setValue("bytes=\(offset)-\(offset + Int64(chunkSize) - 1)", forHTTPHeaderField: "Range")
        }

        let task = session.downloadTask(with: request)
        task.taskDescription = "\(itemID.uuidString)|\(index)|\(offset)"
        task.resume()
    }

    private func startPythonTrack(itemID: UUID, index: Int, track: MediaTrack, destination: URL) {
        try? FileManager.default.removeItem(at: destination)
        Task {
            let backgroundID = await MainActor.run {
                UIApplication.shared.beginBackgroundTask(withName: "Download YouTube media")
            }
            let progressTask = Task { [weak self] in
                let temporary = URL(fileURLWithPath: destination.path + ".download")
                while !Task.isCancelled {
                    guard let self else { return }
                    let partialBytes = Int64(
                        (try? temporary.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
                    )
                    let received = downloadedBytes(itemID: itemID) + partialBytes
                    await MainActor.run {
                        update(itemID) {
                            guard $0.state == .running else { return }
                            $0.receivedBytes = received
                            if let total = $0.totalBytes, total > 0 {
                                $0.progress = min(0.99, Double(received) / Double(total))
                            }
                        }
                    }
                    try? await Task.sleep(nanoseconds: 500_000_000)
                }
            }
            defer {
                progressTask.cancel()
                Task { @MainActor in
                    if backgroundID != .invalid {
                        UIApplication.shared.endBackgroundTask(backgroundID)
                    }
                }
            }
            do {
                try await PythonExtractor.shared.downloadTrack(track, to: destination)
                let canceled = await MainActor.run {
                    items.first(where: { $0.id == itemID })?.state == .canceled
                }
                if canceled {
                    try? FileManager.default.removeItem(at: destination)
                    return
                }
                let received = downloadedBytes(itemID: itemID)
                await MainActor.run {
                    update(itemID) {
                        $0.receivedBytes = received
                        if let total = $0.totalBytes, total > 0 {
                            $0.progress = min(0.99, Double(received) / Double(total))
                        }
                    }
                }
                finishTrack(itemID: itemID, index: index)
            } catch {
                await MainActor.run {
                    update(itemID) {
                        $0.state = .failed
                        $0.errorMessage = error.localizedDescription
                    }
                }
            }
        }
    }

    private func effectiveChunkSize(for track: MediaTrack) -> Int? {
        guard let configuredSize = track.chunkSize, configuredSize > 0 else { return nil }
        let host = track.url.host?.lowercased() ?? ""
        if host == "googlevideo.com" || host.hasSuffix(".googlevideo.com") {
            return nil
        }
        return configuredSize
    }

    private func isGoogleVideo(_ track: MediaTrack) -> Bool {
        let host = track.url.host?.lowercased() ?? ""
        return host == "googlevideo.com" || host.hasSuffix(".googlevideo.com")
    }

    private func finishTrack(itemID: UUID, index: Int) {
        guard var job = jobs[itemID.uuidString] else { return }
        job.completed[index] = true
        jobs[itemID.uuidString] = job
        persistOnMain()

        if let next = job.completed.firstIndex(of: false) {
            startTrack(itemID: itemID, index: next)
        } else {
            finalize(itemID: itemID, job: job)
        }
    }

    private func finalize(itemID: UUID, job: DownloadJob) {
        let saveDestination = job.saveDestination ?? .appFolder
        let outputDirectory = saveDestination == .photoLibrary
            ? Self.photoStagingDirectory
            : Self.documentsDirectory
        let destination = destinationURL(
            directory: outputDirectory,
            suggestedFilename: job.needsMerge ? forceMP4(job.suggestedFilename) : job.suggestedFilename
        )
        let partURLs = job.tracks.indices.map {
            partURL(itemID: itemID, trackIndex: $0, track: job.tracks[$0])
        }

        Task { @MainActor in
            update(itemID) { $0.state = job.needsMerge ? .merging : .running }
            let backgroundID = UIApplication.shared.beginBackgroundTask(withName: "Finish media file")
            Task {
                defer {
                    Task { @MainActor in
                        if backgroundID != .invalid {
                            UIApplication.shared.endBackgroundTask(backgroundID)
                        }
                    }
                }
                do {
                    try FileManager.default.createDirectory(
                        at: outputDirectory,
                        withIntermediateDirectories: true
                    )
                    if job.needsMerge {
                        try await AVFoundationMerger().merge(
                            video: partURLs[0],
                            audio: partURLs[1],
                            output: destination
                        )
                    } else {
                        try FileManager.default.moveItem(at: partURLs[0], to: destination)
                    }
                    if saveDestination == .photoLibrary {
                        await MainActor.run {
                            update(itemID) { $0.state = .saving }
                        }
                        do {
                            try await PhotoLibrarySaver.saveVideo(at: destination)
                            try? FileManager.default.removeItem(at: destination)
                        } catch {
                            try FileManager.default.createDirectory(
                                at: Self.documentsDirectory,
                                withIntermediateDirectories: true
                            )
                            let fallback = destinationURL(
                                directory: Self.documentsDirectory,
                                suggestedFilename: destination.lastPathComponent
                            )
                            try FileManager.default.moveItem(at: destination, to: fallback)
                            removeParts(itemID: itemID)
                            await MainActor.run {
                                update(itemID) {
                                    $0.state = .failed
                                    $0.localFilename = fallback.lastPathComponent
                                    $0.savedDestination = .appFolder
                                    $0.errorMessage = "\(error.localizedDescription) 视频已保存在 YtbDown 文件夹，没有丢失。"
                                }
                                jobs[itemID.uuidString] = nil
                                persist()
                            }
                            return
                        }
                    }
                    removeParts(itemID: itemID)
                    await MainActor.run {
                        update(itemID) {
                            $0.state = .completed
                            $0.progress = 1
                            $0.localFilename = saveDestination == .appFolder
                                ? destination.lastPathComponent
                                : nil
                            $0.savedDestination = saveDestination
                            $0.errorMessage = nil
                        }
                        jobs[itemID.uuidString] = nil
                        persist()
                    }
                } catch {
                    await MainActor.run {
                        update(itemID) {
                            $0.state = .failed
                            $0.errorMessage = error.localizedDescription
                        }
                    }
                }
            }
        }
    }

    private func appendDownloadedFile(_ temporary: URL, to part: URL, replace: Bool) throws -> Int64 {
        try FileManager.default.createDirectory(at: Self.partsDirectory, withIntermediateDirectories: true)
        if replace {
            try? FileManager.default.removeItem(at: part)
            try FileManager.default.moveItem(at: temporary, to: part)
        } else {
            if !FileManager.default.fileExists(atPath: part.path) {
                FileManager.default.createFile(atPath: part.path, contents: nil)
            }
            let output = try FileHandle(forWritingTo: part)
            defer { try? output.close() }
            try output.seekToEnd()
            let input = try FileHandle(forReadingFrom: temporary)
            defer { try? input.close() }
            while let data = try input.read(upToCount: 1_048_576), !data.isEmpty {
                try output.write(contentsOf: data)
            }
        }
        return Int64(try part.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0)
    }

    private func parseContext(_ task: URLSessionTask) -> TaskContext? {
        let parts = task.taskDescription?.split(separator: "|") ?? []
        guard parts.count == 3,
              let id = UUID(uuidString: String(parts[0])),
              let index = Int(parts[1]),
              let offset = Int64(parts[2]) else { return nil }
        return TaskContext(itemID: id, trackIndex: index, startingOffset: offset)
    }

    private func contentRangeTotal(_ response: HTTPURLResponse?) -> Int64? {
        guard let value = response?.value(forHTTPHeaderField: "Content-Range"),
              let suffix = value.split(separator: "/").last,
              suffix != "*" else { return nil }
        return Int64(suffix)
    }

    private func estimatedTotal(_ tracks: [MediaTrack]) -> Int64? {
        let sizes = tracks.map(\.approxBytes)
        guard sizes.allSatisfy({ $0 != nil }) else { return nil }
        return sizes.compactMap { $0 }.reduce(0, +)
    }

    private func downloadedBytes(itemID: UUID) -> Int64 {
        guard let job = jobs[itemID.uuidString] else { return 0 }
        return job.tracks.indices.reduce(0) { total, index in
            let url = partURL(itemID: itemID, trackIndex: index, track: job.tracks[index])
            let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            return total + Int64(size)
        }
    }

    private func destinationURL(directory: URL, suggestedFilename: String) -> URL {
        let cleaned = safeFilename(suggestedFilename)
        let proposed = directory.appendingPathComponent(cleaned)
        guard FileManager.default.fileExists(atPath: proposed.path) else { return proposed }
        let stem = proposed.deletingPathExtension().lastPathComponent
        let ext = proposed.pathExtension
        for index in 2...999 {
            let name = ext.isEmpty ? "\(stem) \(index)" : "\(stem) \(index).\(ext)"
            let candidate = Self.documentsDirectory.appendingPathComponent(name)
            if !FileManager.default.fileExists(atPath: candidate.path) { return candidate }
        }
        return directory.appendingPathComponent("\(UUID().uuidString)-\(cleaned)")
    }

    private func safeFilename(_ value: String) -> String {
        let invalid = CharacterSet(charactersIn: "/:\\?%*|\"<>\n\r")
        let pieces = value.components(separatedBy: invalid).filter { !$0.isEmpty }
        let result = pieces.joined(separator: "-").trimmingCharacters(in: .whitespacesAndNewlines)
        return result.isEmpty ? "video.mp4" : String(result.prefix(180))
    }

    private func forceMP4(_ filename: String) -> String {
        let url = URL(fileURLWithPath: filename)
        return url.deletingPathExtension().lastPathComponent + ".mp4"
    }

    private func removeParts(itemID: UUID) {
        guard let job = jobs[itemID.uuidString] else { return }
        for index in job.tracks.indices {
            try? FileManager.default.removeItem(
                at: partURL(itemID: itemID, trackIndex: index, track: job.tracks[index])
            )
        }
    }

    private func restore() {
        if let data = UserDefaults.standard.data(forKey: itemStorageKey),
           let decoded = try? JSONDecoder().decode([DownloadItem].self, from: data) {
            items = decoded.map { old in
                var item = old
                if item.state == .running || item.state == .queued || item.state == .merging || item.state == .saving {
                    item.state = .failed
                    item.errorMessage = "下载在上次运行时中断，请重新添加链接。"
                }
                return item
            }
        }
        if let data = UserDefaults.standard.data(forKey: jobStorageKey),
           let decoded = try? JSONDecoder().decode([String: DownloadJob].self, from: data) {
            jobs = decoded
        }
    }

    private func reconnectBackgroundTasks() {
        session.getAllTasks { [weak self] tasks in
            guard let self else { return }
            for task in tasks {
                guard let context = self.parseContext(task) else { continue }
                Task { @MainActor in
                    self.update(context.itemID) {
                        $0.state = .running
                        $0.errorMessage = nil
                    }
                }
            }
        }
    }

    @MainActor
    private func update(_ id: UUID, mutation: (inout DownloadItem) -> Void) {
        guard let index = items.firstIndex(where: { $0.id == id }) else { return }
        mutation(&items[index])
        persist()
    }

    @MainActor
    private func persist() {
        if let data = try? JSONEncoder().encode(items) {
            UserDefaults.standard.set(data, forKey: itemStorageKey)
        }
        if let data = try? JSONEncoder().encode(jobs) {
            UserDefaults.standard.set(data, forKey: jobStorageKey)
        }
    }

    private func persistOnMain() {
        Task { @MainActor in persist() }
    }
}

extension DownloadManager: URLSessionDownloadDelegate {
    func urlSessionDidFinishEvents(forBackgroundURLSession session: URLSession) {
        Task { @MainActor in BackgroundSessionCoordinator.shared.finishEvents() }
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        guard let context = parseContext(downloadTask) else { return }
        let completeBytes = downloadedBytes(itemID: context.itemID)
        Task { @MainActor in
            update(context.itemID) {
                $0.state = .running
                $0.receivedBytes = completeBytes + totalBytesWritten
                if let total = $0.totalBytes, total > 0 {
                    $0.progress = min(0.99, Double($0.receivedBytes) / Double(total))
                }
            }
        }
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {
        guard let context = parseContext(downloadTask),
              let job = jobs[context.itemID.uuidString],
              job.tracks.indices.contains(context.trackIndex),
              let response = downloadTask.response as? HTTPURLResponse else { return }
        let track = job.tracks[context.trackIndex]
        let part = partURL(itemID: context.itemID, trackIndex: context.trackIndex, track: track)

        do {
            guard response.statusCode == 200 || response.statusCode == 206 else {
                throw MediaTransferError.httpStatus(response.statusCode)
            }
            let newSize = try appendDownloadedFile(
                location,
                to: part,
                replace: response.statusCode == 200
            )
            let total = contentRangeTotal(response)
            let expectedChunk = Int64(effectiveChunkSize(for: track) ?? 0)
            let received = newSize - context.startingOffset
            let finished = response.statusCode == 200
                || total.map { newSize >= $0 } == true
                || (expectedChunk > 0 && received < expectedChunk)
            if finished {
                finishTrack(itemID: context.itemID, index: context.trackIndex)
            } else {
                startTrack(itemID: context.itemID, index: context.trackIndex)
            }
        } catch {
            Task { @MainActor in
                update(context.itemID) {
                    $0.state = .failed
                    $0.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        didCompleteWithError error: Error?
    ) {
        guard let error, let context = parseContext(task) else { return }
        let nsError = error as NSError
        Task { @MainActor in
            update(context.itemID) {
                if nsError.code == NSURLErrorCancelled, $0.state == .canceled { return }
                $0.state = .failed
                $0.errorMessage = error.localizedDescription
            }
        }
    }
}
