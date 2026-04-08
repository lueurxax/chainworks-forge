import SwiftUI

struct StageDetailView: View {
    let stageExecution: RunStageSnapshot
    let run: Run

    var body: some View {
        WorkflowStageDetailView(stageExecution: stageExecution, run: run)
    }
}
