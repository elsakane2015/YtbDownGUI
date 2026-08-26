import UIKit

final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        handleEventsForBackgroundURLSession identifier: String,
        completionHandler: @escaping () -> Void
    ) {
        guard identifier == DownloadManager.backgroundSessionIdentifier else {
            completionHandler()
            return
        }
        BackgroundSessionCoordinator.shared.completionHandler = completionHandler
    }
}
@MainActor
final class BackgroundSessionCoordinator {
    static let shared = BackgroundSessionCoordinator()
    var completionHandler: (() -> Void)?

    func finishEvents() {
        let handler = completionHandler
        completionHandler = nil
        handler?()
    }
}
