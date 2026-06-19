import Testing
@testable import Chainworks_Forge

@Suite("ForgeGlass", .tags(.fast, .uiModernization))
@MainActor
struct ForgeGlassTests {
    @Test("Liquid Glass roles cover app shell surfaces")
    func liquidGlassRolesCoverAppShellSurfaces() {
        #expect(ForgeGlassRole.allCases == [.panel, .chrome, .sidebar, .toolbar, .prominentAction])
    }

    @Test("Liquid Glass roles expose stable accessibility-neutral identifiers")
    func liquidGlassRolesExposeStableIdentifiers() {
        #expect(ForgeGlassRole.panel.identifier == "panel")
        #expect(ForgeGlassRole.chrome.identifier == "chrome")
        #expect(ForgeGlassRole.sidebar.identifier == "sidebar")
        #expect(ForgeGlassRole.toolbar.identifier == "toolbar")
        #expect(ForgeGlassRole.prominentAction.identifier == "prominent-action")
    }
}
