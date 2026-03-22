import SwiftUI

struct RunProgressView: View {
    let run: Run

    var body: some View {
        WorkflowRunProgressView(run: run)
    }
}
