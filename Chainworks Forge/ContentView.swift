import SwiftUI

extension Notification.Name {
    static let chainworksSelectTab = Notification.Name("chainworks.selectTab")
    static let chainworksOpenRunInRunsHome = Notification.Name("chainworks.openRunInRunsHome")
}

struct ContentView: View {
    @StateObject private var daemonStatus = DaemonStatusViewModel.bootstrap()
    @StateObject private var schedulerHealth = SchedulerHealthViewModel.bootstrap()

    var body: some View {
        VStack(spacing: 0) {
            if daemonStatus.shouldDisplayBanner || schedulerHealth.bannerIssue != nil {
                DaemonLifecycleBanner(
                    viewModel: daemonStatus,
                    schedulerHealthIssue: schedulerHealth.bannerIssue,
                    onOpenSchedulerHealth: {}
                )
                .padding(.horizontal, 12)
                .padding(.top, 8)
            }
            ControlPlaneOnlyPlaceholder(
                title: "Control-plane UI cutover",
                message: "SwiftData-backed operator UI has been intentionally removed. Rebuild screens on GraphQL read projections only; command/control remains outside UI-owned database paths."
            )
        }
        .task {
            await daemonStatus.startSnapshotPlusSubscribe()
        }
        .task {
            await schedulerHealth.refresh()
        }
    }
}

struct ControlPlaneOnlyPlaceholder: View {
    let title: String
    var message: String = "This legacy SwiftData surface was removed during the control-plane UI cutover."

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.title2.weight(.semibold))
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(24)
    }
}

#Preview {
    ContentView()
}
