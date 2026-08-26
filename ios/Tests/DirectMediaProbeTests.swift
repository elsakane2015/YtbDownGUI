import XCTest
@testable import YtbDownIOS

final class DirectMediaProbeTests: XCTestCase {
    func testAcceptsHTTPSDirectURL() throws {
        let url = try DirectMediaProbe.validate(text: "https://cdn.example.com/video.mp4")
        XCTAssertEqual(url.absoluteString, "https://cdn.example.com/video.mp4")
    }

    func testRejectsHTTP() {
        XCTAssertThrowsError(try DirectMediaProbe.validate(text: "http://cdn.example.com/video.mp4")) {
            XCTAssertEqual($0 as? DownloadEngineError, .insecureURL)
        }
    }

    func testRejectsKnownVideoPage() {
        XCTAssertThrowsError(try DirectMediaProbe.validate(text: "https://www.youtube.com/watch?v=abc")) {
            XCTAssertEqual($0 as? DownloadEngineError, .unsupportedPage)
        }
    }
}
