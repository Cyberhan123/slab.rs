from enum import Enum


class WorkspaceWatchEventType(str, Enum):
    CHANGED = "changed"
    CREATED = "created"
    DELETED = "deleted"

    def __str__(self) -> str:
        return str(self.value)
