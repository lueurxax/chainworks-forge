import SwiftUI

struct DefinitionsView: View {
    @State private var selectedSegment: Segment = .agents
    @Binding var segmentRequest: Segment?

    let catalogURL: URL?
    let workflowURL: URL?
    let compactWorkflowURL: URL?

    enum Segment: String, CaseIterable {
        case agents = "Agents"
        case workflows = "Workflows"
    }

    init(
        catalogURL: URL?,
        workflowURL: URL?,
        compactWorkflowURL: URL?,
        segmentRequest: Binding<Segment?> = .constant(nil)
    ) {
        self.catalogURL = catalogURL
        self.workflowURL = workflowURL
        self.compactWorkflowURL = compactWorkflowURL
        self._segmentRequest = segmentRequest
    }

    var body: some View {
        VStack(spacing: 0) {
            Picker("Segment", selection: $selectedSegment) {
                ForEach(Segment.allCases, id: \.self) { segment in
                    Text(segment.rawValue).tag(segment)
                }
            }
            .pickerStyle(.segmented)
            .padding()

            Divider()

            Group {
                switch selectedSegment {
                case .agents:
                    AgentCatalogView(catalogURL: catalogURL)
                case .workflows:
                    WorkflowInspectorView(
                        workflowURL: workflowURL,
                        compactWorkflowURL: compactWorkflowURL,
                        catalogURL: catalogURL
                    )
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityIdentifier("definitions-view")
        // onAppear handles the initial value set before the view is created (e.g. from ContentView
        // init or a notification that fires before DefinitionsView is first displayed).
        .onAppear {
            if let seg = segmentRequest {
                selectedSegment = seg
                segmentRequest = nil
            }
        }
        .onChange(of: segmentRequest) {
            if let seg = segmentRequest {
                selectedSegment = seg
                segmentRequest = nil
            }
        }
    }
}
