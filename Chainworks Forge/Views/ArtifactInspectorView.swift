import SwiftUI

struct ArtifactInspectorView: View {
    let artifact: Artifact
    let run: Run

    var body: some View {
        WorkflowArtifactInspectorView(run: run, artifact: artifact)
    }
}
