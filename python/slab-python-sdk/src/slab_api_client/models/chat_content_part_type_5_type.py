from enum import Enum


class ChatContentPartType5Type(str, Enum):
    JSON = "json"

    def __str__(self) -> str:
        return str(self.value)
