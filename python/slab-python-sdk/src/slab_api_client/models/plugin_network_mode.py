from enum import Enum


class PluginNetworkMode(str, Enum):
    ALLOWLIST = "allowlist"
    BLOCKED = "blocked"

    def __str__(self) -> str:
        return str(self.value)
