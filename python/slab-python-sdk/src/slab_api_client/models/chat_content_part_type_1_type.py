from enum import Enum


class ChatContentPartType1Type(str, Enum):
    INPUT_TEXT = "input_text"

    def __str__(self) -> str:
        return str(self.value)
