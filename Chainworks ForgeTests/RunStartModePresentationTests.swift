import Testing
@testable import Chainworks_Forge

// RunStartMode and RunStartModePresentationPolicy were removed as out-of-scope for P036.
// P036 keeps Ideas read-only and does not introduce run-start controls.
@Suite("RunStartModePresentation", .tags(.fast))
struct RunStartModePresentationTests {}
