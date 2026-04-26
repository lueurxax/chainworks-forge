import Testing
@testable import Chainworks_Forge

@MainActor
struct AppTerminationCoordinatorTests {
    private final class ExecutionTerminationControllerSpy: ExecutionTerminationControlling {
        private(set) var prepareForTerminationCallCount = 0

        func prepareForTermination() {
            prepareForTerminationCallCount += 1
        }
    }

    @Test("App termination coordinator invokes execution termination controller on termination")
    func coordinatorInvokesExecutionTermination() {
        let executionSpy = ExecutionTerminationControllerSpy()
        let coordinator = AppTerminationCoordinator()
        coordinator.executionTerminationController = executionSpy

        coordinator.prepareForTermination()

        #expect(executionSpy.prepareForTerminationCallCount == 1)
    }

    @Test("App termination coordinator tolerates missing execution service")
    func coordinatorAllowsMissingExecutionService() {
        let coordinator = AppTerminationCoordinator()

        coordinator.prepareForTermination()

        #expect(Bool(true))
    }
}
