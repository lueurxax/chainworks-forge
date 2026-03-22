import SwiftUI
import SwiftData

struct IdeaListView: View {
    @Environment(\.modelContext) private var modelContext
    @Query(sort: \Idea.createdAt, order: .reverse) private var ideas: [Idea]
    @State private var newTitle = ""
    @State private var newBody = ""
    @State private var newAttachmentPath = ""

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                // Summary strip (UI-001)
                summaryStrip

                Group {
                    if ideas.isEmpty {
                        ContentUnavailableView(
                            "No ideas yet",
                            systemImage: "lightbulb",
                            description: Text("Create your first idea to get started.")
                        )
                    } else {
                        List {
                            ForEach(ideas) { idea in
                                NavigationLink {
                                    IdeaDetailView(idea: idea)
                                } label: {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(idea.title).font(.headline)
                                        HStack {
                                            Text(idea.status.rawValue.capitalized)
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
                                            if idea.attachmentPath != nil {
                                                Image(systemName: "paperclip")
                                                    .font(.caption2)
                                                    .foregroundStyle(.secondary)
                                            }
                                        }
                                    }
                                }
                            }
                            .onDelete(perform: deleteIdeas)
                        }
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 200, ideal: 250)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { showNewIdeaSheet = true }) {
                        Label("New Idea", systemImage: "plus")
                    }
                }
            }
            .sheet(isPresented: $showNewIdeaSheet) {
                newIdeaSheet
            }
        } detail: {
            Text("Select an idea")
                .foregroundStyle(.secondary)
        }
    }

    @State private var showNewIdeaSheet = false

    // MARK: - Summary Strip

    private var summaryStrip: some View {
        let draftCount = ideas.filter { $0.status == .draft }.count
        let activeCount = ideas.filter { $0.status == .active }.count

        return HStack {
            Image(systemName: "lightbulb.fill")
                .foregroundStyle(.blue)
            Text("\(ideas.count) ideas · \(draftCount) drafts · \(activeCount) active")
            Spacer()
        }
        .font(.caption)
        .padding(.horizontal)
        .padding(.vertical, 6)
        .background(Color.blue.opacity(0.08))
    }

    // MARK: - New Idea Sheet

    private var newIdeaSheet: some View {
        VStack(spacing: 16) {
            Text("New Idea").font(.headline)
            TextField("Title", text: $newTitle)
                .textFieldStyle(.roundedBorder)
            TextEditor(text: $newBody)
                .frame(minHeight: 100)
                .border(Color.secondary.opacity(0.3))
            HStack {
                TextField("Attachment path (optional)", text: $newAttachmentPath)
                    .textFieldStyle(.roundedBorder)
                Button("Browse...") { browseAttachment() }
            }
            HStack {
                Button("Cancel") {
                    resetForm()
                    showNewIdeaSheet = false
                }
                Spacer()
                Button("Save Idea") {
                    createIdea()
                    showNewIdeaSheet = false
                }
                .disabled(newTitle.trimmingCharacters(in: .whitespaces).isEmpty)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding()
        .frame(minWidth: 400, minHeight: 300)
    }

    private func createIdea() {
        let trimmedPath = newAttachmentPath.trimmingCharacters(in: .whitespaces)
        let idea = Idea(
            title: newTitle.trimmingCharacters(in: .whitespaces),
            body: newBody.trimmingCharacters(in: .whitespaces),
            attachmentPath: trimmedPath.isEmpty ? nil : trimmedPath
        )
        modelContext.insert(idea)
        resetForm()
    }

    private func resetForm() {
        newTitle = ""
        newBody = ""
        newAttachmentPath = ""
    }

    private func deleteIdeas(offsets: IndexSet) {
        for index in offsets {
            modelContext.delete(ideas[index])
        }
    }

    private func browseAttachment() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            newAttachmentPath = url.path
        }
        #endif
    }
}

struct IdeaDetailView: View {
    let idea: Idea

    var body: some View {
        Form {
            Section("Idea") {
                LabeledContent("Title", value: idea.title)
                LabeledContent("Status", value: idea.status.rawValue.capitalized)
                LabeledContent("Created", value: idea.createdAt, format: .dateTime)
                if let path = idea.attachmentPath {
                    LabeledContent("Attachment", value: path)
                }
            }
            Section("Body") {
                Text(idea.body)
                    .textSelection(.enabled)
            }
            Section("Runs") {
                if idea.runs.isEmpty {
                    Text("No runs yet")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(idea.runs) { run in
                        LabeledContent(run.workflowTitle, value: run.status.rawValue)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(idea.title)
    }
}
