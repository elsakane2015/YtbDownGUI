import SwiftUI

struct TasksView: View {
    @EnvironmentObject private var downloadManager: DownloadManager

    var body: some View {
        Group {
            if downloadManager.items.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "arrow.down.doc")
                        .font(.system(size: 42))
                        .foregroundStyle(.secondary)
                    Text("还没有下载任务")
                        .font(.title3.bold())
                    Text("解析视频并选择清晰度后，任务会出现在这里。")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .padding()
            } else {
                List {
                    ForEach(downloadManager.items) { item in
                        DownloadRow(item: item)
                            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                                Button(role: .destructive) {
                                    downloadManager.removeRecord(item)
                                } label: {
                                    Label("移除记录", systemImage: "trash")
                                }
                            if item.state == .running || item.state == .queued || item.state == .merging {
                                    Button {
                                        downloadManager.cancel(item)
                                    } label: {
                                        Label("取消", systemImage: "xmark")
                                    }
                                    .tint(.orange)
                                }
                            }
                    }
                }
                .listStyle(.insetGrouped)
            }
        }
        .navigationTitle("下载任务")
    }
}

private struct DownloadRow: View {
    @EnvironmentObject private var downloadManager: DownloadManager
    @State private var isShowingShareSheet = false
    let item: DownloadItem

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(item.title)
                .font(.headline)
                .lineLimit(2)

            if item.state == .running || item.state == .queued || item.state == .merging || item.state == .saving {
                ProgressView(value: item.progress)
                HStack {
                    Text(item.state.title)
                    Spacer()
                    Text(progressText)
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            } else {
                HStack {
                    Label(item.state.title, systemImage: stateIcon)
                        .foregroundStyle(stateColor)
                    Spacer()
                    if let file = downloadManager.localFileURL(for: item) {
                        Button {
                            isShowingShareSheet = true
                        } label: {
                            Label("分享", systemImage: "square.and.arrow.up")
                        }
                    }
                }
                .font(.subheadline)

                if item.state == .completed, let destination = item.savedDestination {
                    Label(destination.detail, systemImage: destination == .photoLibrary ? "photo" : "folder")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            if let error = item.errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
        .padding(.vertical, 5)
        .sheet(isPresented: $isShowingShareSheet) {
            if let file = downloadManager.localFileURL(for: item) {
                ActivityView(activityItems: [file])
                    .presentationDetents([.medium, .large])
            }
        }
    }

    private var progressText: String {
        let received = ByteCountFormatter.string(fromByteCount: item.receivedBytes, countStyle: .file)
        guard let total = item.totalBytes, total > 0 else { return received }
        let totalText = ByteCountFormatter.string(fromByteCount: total, countStyle: .file)
        return "\(received) / \(totalText)"
    }

    private var stateIcon: String {
        switch item.state {
        case .completed: return "checkmark.circle.fill"
        case .failed: return "exclamationmark.circle.fill"
        case .canceled: return "xmark.circle"
        case .queued, .running: return "arrow.down.circle"
        case .merging: return "arrow.triangle.2.circlepath.circle"
        case .saving: return "square.and.arrow.down"
        }
    }

    private var stateColor: Color {
        switch item.state {
        case .completed: return .green
        case .failed: return .red
        case .canceled: return .secondary
        case .queued, .running, .merging, .saving: return .blue
        }
    }
}

private struct ActivityView: UIViewControllerRepresentable {
    let activityItems: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
