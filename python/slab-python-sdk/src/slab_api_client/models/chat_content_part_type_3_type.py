from enum import Enum


class ChatContentPartType3Type(str, Enum):
    IMAGE = "image"

    def __str__(self) -> str:
        return str(self.value)
