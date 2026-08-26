import SwiftUI

@main
struct YtbDownIOSApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var downloadManager = DownloadManager()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(downloadManager)
        }
    }
}
