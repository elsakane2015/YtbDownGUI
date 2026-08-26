import XCTest
@testable import YtbDownIOS

final class ExtractionDecoderTests: XCTestCase {
    func testDecodesAdaptiveMediaAndHeaders() throws {
        let json = #"{"ok":true,"kind":"adaptive","title":"Demo","filename":"Demo.mp4","video":{"url":"https://example.com/video","headers":{"Referer":"https://example.com"},"vcodec":"avc1.640028","ext":"mp4","chunk_size":1048576,"approx_bytes":2000},"audio":{"url":"https://example.com/audio","headers":{},"acodec":"mp4a.40.2","ext":"m4a","approx_bytes":1000}}"#
        let media = try ExtractionDecoder.decode(Data(json.utf8))

        guard case let .adaptive(video, audio) = media.kind else {
            return XCTFail("Expected adaptive media")
        }
        XCTAssertEqual(media.title, "Demo")
        XCTAssertEqual(video.httpHeaders["Referer"], "https://example.com")
        XCTAssertEqual(video.chunkSize, 1_048_576)
        XCTAssertEqual(audio.codec, "mp4a.40.2")
    }

    func testDecodesQualityChoices() throws {
        let json = #"{"ok":true,"kind":"choices","options":[{"height":1080,"codec":"H.264","approx_bytes":50000000,"format_id":"137","adaptive":true},{"height":720,"codec":"H.264","format_id":"22","adaptive":false}]}"#
        let listing = try ExtractionDecoder.decodeListing(Data(json.utf8))

        guard case let .choices(options) = listing else {
            return XCTFail("Expected quality choices")
        }
        XCTAssertEqual(options.map(\.height), [1080, 720])
        XCTAssertTrue(options[0].isAdaptive)
        XCTAssertFalse(options[1].isAdaptive)
    }

    func testMapsAuthenticationFailure() {
        let json = #"{"ok":false,"error_kind":"requires_auth","detail":"login required"}"#
        XCTAssertThrowsError(try ExtractionDecoder.decode(Data(json.utf8))) { error in
            XCTAssertEqual(error as? LocalExtractionError, .requiresLogin)
        }
    }

    func testLiveYouTubeExtractionWhenEnabled() async throws {
        guard ProcessInfo.processInfo.environment["RUN_LIVE_EXTRACTION"] == "1" else {
            throw XCTSkip("在 Xcode 的测试 Scheme 中设置 RUN_LIVE_EXTRACTION=1 后运行。")
        }
        let url = try XCTUnwrap(URL(string: "https://www.youtube.com/watch?v=dQw4w9WgXcQ"))
        let listing = try await PythonExtractor.shared.listFormats(url)
        let media: ResolvedMedia
        switch listing {
        case .ready(let media):
            self.assertUsable(media)
            return
        case .choices(let options):
            XCTAssertFalse(options.isEmpty)
            media = try await PythonExtractor.shared.resolve(url, option: try XCTUnwrap(options.last))
        }
        assertUsable(media)
    }

    func testLiveYouTubeTransferOnDeviceWhenEnabled() async throws {
        guard ProcessInfo.processInfo.environment["RUN_LIVE_TRANSFER"] == "1" else {
            throw XCTSkip("设置 RUN_LIVE_TRANSFER=1 后在真机运行。")
        }
        let url = try XCTUnwrap(URL(string: "https://youtu.be/S8Cz71rvIwg"))
        let listing = try await PythonExtractor.shared.listFormats(url)
        let media: ResolvedMedia
        switch listing {
        case .ready(let ready):
            media = ready
        case .choices(let options):
            guard let option = options.first(where: { $0.height == 360 }) ?? options.last else {
                return XCTFail("没有找到可用格式")
            }
            media = try await PythonExtractor.shared.resolve(url, option: option)
        }
        let tracks: [MediaTrack]
        switch media.kind {
        case .progressive(let track): tracks = [track]
        case .adaptive(let video, let audio): tracks = [video, audio]
        }

        for (index, track) in tracks.enumerated() {
            let temporary = FileManager.default.temporaryDirectory
                .appendingPathComponent("ytbdown-live-\(UUID().uuidString)-\(index)")
            try await PythonExtractor.shared.downloadTrack(track, to: temporary)
            let attributes = try FileManager.default.attributesOfItem(atPath: temporary.path)
            XCTAssertGreaterThan((attributes[.size] as? NSNumber)?.intValue ?? 0, 1_048_576)
            if FileManager.default.fileExists(atPath: temporary.path) {
                try? FileManager.default.removeItem(at: temporary)
            }
        }
    }

    private func assertUsable(_ media: ResolvedMedia) {
        XCTAssertFalse(media.suggestedFilename.isEmpty)
        switch media.kind {
        case .progressive(let track):
            XCTAssertEqual(track.url.scheme, "https")
        case .adaptive(let video, let audio):
            XCTAssertEqual(video.url.scheme, "https")
            XCTAssertEqual(audio.url.scheme, "https")
            XCTAssertFalse(video.httpHeaders.isEmpty)
            XCTAssertFalse(audio.httpHeaders.isEmpty)
        }
    }
}
