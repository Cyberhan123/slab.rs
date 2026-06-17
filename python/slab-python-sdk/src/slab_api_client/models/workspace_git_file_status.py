from enum import Enum


class WorkspaceGitFileStatus(str, Enum):
    ADDED = "added"
    CONFLICTED = "conflicted"
    COPIED = "copied"
    DELETED = "deleted"
    MODIFIED = "modified"
    RENAMED = "renamed"
    UNTRACKED = "untracked"

    def __str__(self) -> str:
        return str(self.value)
