import SwiftUI

struct SettingsView: View {
    @AppStorage(SaveDestination.storageKey) private var saveDestinationRawValue = SaveDestination.appFolder.rawValue

    var body: some View {
        List {
            Section {
                Picker("下载后保存到", selection: $saveDestinationRawValue) {
                    ForEach(SaveDestination.allCases) { destination in
                        VStack(alignment: .leading) {
                            Text(destination.title)
                            Text(destination.detail)
                        }
                        .tag(destination.rawValue)
                    }
                }
                .pickerStyle(.inline)
            } header: {
                Text("保存位置")
            } footer: {
                Text("仅影响新添加的下载任务。首次保存到系统相册时，iPhone 会询问照片写入权限。")
            }

            Section("下载引擎") {
                LabeledContent("当前模式", value: "iPhone 完全本地")
                LabeledContent("页面解析", value: "内嵌 yt-dlp")
                LabeledContent("音视频合并", value: "AVFoundation")
            }

            Section("当前限制") {
                Text("优先支持公开内容与 H.264/HEVC + AAC，通常最高 1080p；暂不支持 DRM、账号登录、播放列表批量和需要 ffmpeg 的编码。")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Section("版本") {
                LabeledContent("iOS App", value: versionText)
                LabeledContent("最低系统", value: "iOS 16")
                LabeledContent("安装方式", value: "付费开发签名（自用）")
            }

            Section {
                Text("仅下载你有权保存的内容，并遵守来源网站的服务条款。")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .navigationTitle("设置")
    }

    private var versionText: String {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "-"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "-"
        return "v\(version) (\(build))"
    }
}
