import SwiftUI

struct DefinitionsView: View {
    @State private var selectedSegment: Segment = .agents
    
    let catalogURL: URL?
    let workflowURL: URL?
    let compactWorkflowURL: URL?
    
    enum Segment: String, CaseIterable {
        case agents = "Agents"
        case workflows = "Workflows"
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
    }
}
