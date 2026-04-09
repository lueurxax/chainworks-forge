import Testing
@testable import Chainworks_Forge

@MainActor
struct AppTerminationCoordinatorTests {
    private final class ManagedGooseServerControllerSpy: ManagedGooseServerControlling {
        private(set) var stopManagedServerCallCount = 0

        func stopManagedServer() {
            stopManagedServerCallCount += 1
        }
    }

    private final class ExecutionTerminationControllerSpy: ExecutionTerminationControlling {
        private(set) var prepareForTerminationCallCount = 0

        func prepareForTermination() {
            prepareForTerminationCallCount += 1
        }
    }

    @Test("App termination coordinator stops managed Goose server on termination")
    func coordinatorStopsManagedGooseServer() {
        let spy = ManagedGooseServerControllerSpy()
        let executionSpy = ExecutionTerminationControllerSpy()
        let coordinator = AppTerminationCoordinator()
        coordinator.gooseServerManager = spy
        coordinator.executionTerminationController = executionSpy

        coordinator.prepareForTermination()

        #expect(spy.stopManagedServerCallCount == 1)
        #expect(executionSpy.prepareForTerminationCallCount == 1)
    }

    @Test("App termination coordinator tolerates missing Goose server manager")
    func coordinatorAllowsMissingManager() {
        let coordinator = AppTerminationCoordinator()

        coordinator.prepareForTermination()

        #expect(Bool(true))
    }

    @Test("App termination coordinator retains Goose server manager until termination")
    func coordinatorRetainsManagerUntilTermination() {
        let coordinator = AppTerminationCoordinator()
        var spy: ManagedGooseServerControllerSpy? = ManagedGooseServerControllerSpy()
        weak var weakSpy = spy

        coordinator.gooseServerManager = spy
        spy = nil

        #expect(weakSpy != nil)

        coordinator.prepareForTermination()

        #expect(weakSpy?.stopManagedServerCallCount == 1)
    }

    @Test("App termination coordinator tolerates missing execution service")
    func coordinatorAllowsMissingExecutionService() {
        let spy = ManagedGooseServerControllerSpy()
        let coordinator = AppTerminationCoordinator()
        coordinator.gooseServerManager = spy

        coordinator.prepareForTermination()

        #expect(spy.stopManagedServerCallCount == 1)
    }
}
