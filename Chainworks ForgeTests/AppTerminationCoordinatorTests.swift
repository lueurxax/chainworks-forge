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

    @Test("Graceful termination waits through terminateLater path and replies once")
    func gracefulTerminationRepliesAfterBoundedPreparation() async {
        let executionSpy = ExecutionTerminationControllerSpy()
        let coordinator = AppTerminationCoordinator()
        coordinator.hostTotalMilliseconds = 0
        coordinator.executionTerminationController = executionSpy
        var replies: [Bool] = []

        coordinator.beginGracefulTermination { shouldTerminate in
            replies.append(shouldTerminate)
        }

        await Task.yield()

        #expect(executionSpy.prepareForTerminationCallCount == 1)
        #expect(replies == [true])
    }

    @Test("Graceful termination ignores duplicate AppKit callbacks while pending")
    func gracefulTerminationIgnoresDuplicateCallbacks() async {
        let executionSpy = ExecutionTerminationControllerSpy()
        let coordinator = AppTerminationCoordinator()
        coordinator.hostTotalMilliseconds = 1
        coordinator.executionTerminationController = executionSpy
        var replyCount = 0

        coordinator.beginGracefulTermination { _ in
            replyCount += 1
        }
        coordinator.beginGracefulTermination { _ in
            replyCount += 1
        }

        try? await Task.sleep(nanoseconds: 2_000_000)

        #expect(executionSpy.prepareForTerminationCallCount == 1)
        #expect(replyCount == 1)
    }
}
