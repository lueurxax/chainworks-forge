import SwiftUI

struct StageDetailView: View {
    let stageExecution: StageExecution
    let run: Run

    var body: some View {
        WorkflowStageDetailView(stageExecution: stageExecution, run: run)
    }
}
