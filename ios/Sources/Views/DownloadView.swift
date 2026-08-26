import SwiftUI
import UIKit

struct DownloadView: View {
    @EnvironmentObject private var downloadManager: DownloadManager
    @State private var urlText = ""
    @State private var isInspecting = false
    @State private var message: String?
    @State private var formatOptions: [FormatOption] = []
    @State private var pendingURL: URL?
    @State private var showQualityPicker = false
    @FocusState private var urlFocused: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("保存媒体到 iPhone")
                        .font(.largeTitle.bold())
                    Text("粘贴视频页面链接。解析、下载和音视频合并全部在这台 iPhone 内完成。")
                        .foregroundStyle(.secondary)
                }

                VStack(spacing: 12) {
                    TextField("粘贴 YouTube 或其他视频页面链接", text: $urlText, axis: .vertical)
                        .textInputAutocapitalization(.never)
                        .keyboardType(.URL)
                        .autocorrectionDisabled()
                        .focused($urlFocused)
                        .lineLimit(2...4)
                        .padding(14)
                        .background(.secondary.opacity(0.12), in: RoundedRectangle(cornerRadius: 12))

                    HStack {
                        Button {
                            urlText = UIPasteboard.general.string ?? ""
                        } label: {
                            Label("粘贴", systemImage: "doc.on.clipboard")
                        }
                        .buttonStyle(.bordered)

                        Spacer()

                        Button {
                            inspectAndDownload()
                        } label: {
                            if isInspecting {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Label("解析视频", systemImage: "arrow.down")
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(isInspecting || urlText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
                .padding(16)
                .background(.background, in: RoundedRectangle(cornerRadius: 18))
                .shadow(color: .black.opacity(0.06), radius: 16, y: 6)

                if let message {
                    Label(message, systemImage: "exclamationmark.triangle.fill")
                        .font(.callout)
                        .foregroundStyle(.orange)
                        .padding(14)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(.orange.opacity(0.1), in: RoundedRectangle(cornerRadius: 12))
                }

                VStack(alignment: .leading, spacing: 10) {
                    Label("完全本地处理", systemImage: "iphone.and.arrow.forward")
                        .font(.headline)
                    Text("App 内嵌页面解析器，下载使用 iOS 后台传输；分离的视频与音频使用系统媒体框架合并，不会把链接或文件传到中转服务器。当前优先支持公开内容和 H.264/HEVC + AAC 格式，通常最高到 1080p。")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(16)
                .background(.blue.opacity(0.08), in: RoundedRectangle(cornerRadius: 14))
            }
            .padding()
        }
        .background(Color(uiColor: .systemGroupedBackground))
        .navigationTitle("YtbDown")
        .navigationBarTitleDisplayMode(.inline)
        .sheet(isPresented: $showQualityPicker) {
            NavigationStack {
                List(formatOptions) { option in
                    Button {
                        resolveAndDownload(option)
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(option.displayLabel)
                                    .foregroundStyle(.primary)
                                if option.isAdaptive {
                                    Text("下载后在 iPhone 本地合并")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Spacer()
                            Image(systemName: "arrow.down.circle.fill")
                                .font(.title3)
                        }
                    }
                }
                .navigationTitle("选择清晰度")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("取消") { showQualityPicker = false }
                    }
                }
            }
            .presentationDetents([.medium, .large])
        }
    }

    private func inspectAndDownload() {
        urlFocused = false
        message = nil
        isInspecting = true
        Task {
            do {
                let url = try validatedPageURL(urlText)
                let listing = try await PythonExtractor.shared.listFormats(url)
                switch listing {
                case .ready(let media):
                    downloadManager.enqueue(media, sourceURL: url)
                    urlText = ""
                case .choices(let options):
                    pendingURL = url
                    formatOptions = options
                    showQualityPicker = true
                }
            } catch {
                message = error.localizedDescription
            }
            isInspecting = false
        }
    }

    private func resolveAndDownload(_ option: FormatOption) {
        guard let url = pendingURL else { return }
        showQualityPicker = false
        isInspecting = true
        message = nil
        Task {
            do {
                let media = try await PythonExtractor.shared.resolve(url, option: option)
                downloadManager.enqueue(media, sourceURL: url)
                urlText = ""
                pendingURL = nil
                formatOptions = []
            } catch {
                message = error.localizedDescription
            }
            isInspecting = false
        }
    }

    private func validatedPageURL(_ text: String) throws -> URL {
        let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: value),
              let scheme = url.scheme?.lowercased(),
              scheme == "https",
              url.host != nil else {
            throw DownloadEngineError.invalidURL
        }
        return url
    }
}
