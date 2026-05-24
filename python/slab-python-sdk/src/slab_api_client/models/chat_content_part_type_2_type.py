from enum import Enum


class ChatContentPartType2Type(str, Enum):
    OUTPUT_TEXT = "output_text"

    def __str__(self) -> str:
        return str(self.value)
