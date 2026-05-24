from enum import Enum


class ChatModelSource(str, Enum):
    CLOUD = "cloud"
    LOCAL = "local"

    def __str__(self) -> str:
        return str(self.value)
