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
            RunsHomeView()
        }
        .task {
            await daemonStatus.startSnapshotPlusSubscribe()
        }
        .task {
            await schedulerHealth.refresh()
        }
    }
}

#Preview {
    ContentView()
}
