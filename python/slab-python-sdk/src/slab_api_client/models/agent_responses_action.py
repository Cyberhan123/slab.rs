from enum import Enum


class AgentResponsesAction(str, Enum):
    APPROVAL_RESOLVE = "approval_resolve"
    INPUT = "input"
    INTERRUPT = "interrupt"
    RESPONSE_CREATE = "response_create"
    SESSION_RESTORE = "session_restore"
    SHUTDOWN = "shutdown"

    def __str__(self) -> str:
        return str(self.value)
