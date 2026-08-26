import SwiftUI

struct RootView: View {
    var body: some View {
        TabView {
            NavigationStack {
                DownloadView()
            }
            .tabItem { Label("下载", systemImage: "arrow.down.circle") }

            NavigationStack {
                TasksView()
            }
            .tabItem { Label("任务", systemImage: "list.bullet.rectangle") }

            NavigationStack {
                SettingsView()
            }
            .tabItem { Label("设置", systemImage: "gearshape") }
        }
        .tint(.blue)
    }
}
