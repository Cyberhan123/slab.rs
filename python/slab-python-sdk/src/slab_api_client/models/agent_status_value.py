from enum import Enum


class AgentStatusValue(str, Enum):
    COMPLETED = "completed"
    ERRORED = "errored"
    INTERRUPTED = "interrupted"
    INTERRUPTING = "interrupting"
    PENDING = "pending"
    RUNNING = "running"
    SHUTDOWN = "shutdown"

    def __str__(self) -> str:
        return str(self.value)
