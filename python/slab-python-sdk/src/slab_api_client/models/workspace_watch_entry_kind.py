from enum import Enum


class WorkspaceWatchEntryKind(str, Enum):
    DIRECTORY = "directory"
    FILE = "file"
    UNKNOWN = "unknown"

    def __str__(self) -> str:
        return str(self.value)
