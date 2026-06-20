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

    @Test("Graceful termination exposes canonical bounded shutdown budgets")
    func gracefulTerminationExposesCanonicalBudgets() {
        #expect(AppTerminationCoordinator.hostTotalMs == 30_000)
        #expect(AppTerminationCoordinator.receiptFlushTailMs == 1_000)
    }

    @Test("Graceful termination preparation is idempotent at coordinator boundary")
    func gracefulTerminationPreparationCanBeCalledRepeatedly() {
        let executionSpy = ExecutionTerminationControllerSpy()
        let coordinator = AppTerminationCoordinator()
        coordinator.executionTerminationController = executionSpy

        coordinator.prepareForTermination()
        coordinator.prepareForTermination()

        #expect(executionSpy.prepareForTerminationCallCount == 2)
    }
}
