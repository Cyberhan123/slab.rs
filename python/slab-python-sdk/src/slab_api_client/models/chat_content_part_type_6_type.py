from enum import Enum


class ChatContentPartType6Type(str, Enum):
    REFUSAL = "refusal"

    def __str__(self) -> str:
        return str(self.value)
