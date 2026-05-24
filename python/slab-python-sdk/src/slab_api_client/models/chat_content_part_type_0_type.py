from enum import Enum


class ChatContentPartType0Type(str, Enum):
    TEXT = "text"

    def __str__(self) -> str:
        return str(self.value)
